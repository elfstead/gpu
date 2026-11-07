//! Cold ray linkage and replay metadata. No library/resource retention or global policy.
use super::*;
pub(super) struct Linkage {
    pub libraries: Vec<vk::VkPipeline>,
    pub interface: Option<vk::VkRayTracingPipelineInterfaceCreateInfoKHR>,
    pub stages: Vec<u32>,
    pub imported_groups: Vec<u8>,
    pub replay: Vec<*const std::ffi::c_void>,
}
impl Linkage {
    pub unsafe fn new(
        d: &Device,
        state: &RayState,
        shaders: &[Shader],
        library: bool,
    ) -> Result<Self, Status> {
        let mut info = None;
        let mut replay = None;
        let mut next = state.header.next;
        let mut count = 0;
        while !next.is_null() {
            count += 1;
            if count > 2 {
                return Err(INVALID);
            }
            let header = unsafe { &*next };
            match header.kind {
                RAY_LIBRARIES_INFO if info.is_none() => {
                    info = Some(unsafe {
                        local_read::parent::<RayLibrariesInfo>(next, RAY_LIBRARIES_INFO)?
                    })
                }
                RAY_REPLAY_INFO if replay.is_none() => {
                    replay =
                        Some(unsafe { local_read::parent::<RayReplayInfo>(next, RAY_REPLAY_INFO)? })
                }
                RAY_LIBRARIES_INFO | RAY_REPLAY_INFO => return Err(INVALID),
                _ => return Err(UNSUPPORTED),
            }
            next = header.next;
        }
        if library && info.is_none() {
            return Err(INVALID);
        }
        if state.flags & RAY_CAPTURE_HANDLES != 0
            && d.snapshot.features.enabled & RAY_CAPTURE_REPLAY == 0
        {
            return Err(UNSUPPORTED);
        }
        let mut result = Self {
            libraries: Vec::new(),
            interface: None,
            stages: Vec::new(),
            imported_groups: Vec::new(),
            replay: Vec::new(),
        };
        result
            .stages
            .try_reserve_exact(shaders.len())
            .map_err(|_| OUT_OF_MEMORY)?;
        result.stages.extend(shaders.iter().map(|s| s.stage));
        if let Some(info) = info {
            if d.snapshot.features.enabled & RAY_LIBRARIES == 0 {
                return Err(UNSUPPORTED);
            }
            if info.reserved != 0 {
                return Err(INVALID);
            }
            if info.max_hit_attribute_size > d.snapshot.ray_limits.max_hit_attribute_size {
                return Err(UNSUPPORTED);
            }
            let libraries = unsafe { commands::slice(info.libraries, info.count)? };
            result
                .libraries
                .try_reserve_exact(libraries.len())
                .map_err(|_| OUT_OF_MEMORY)?;
            for library in libraries {
                let e = unsafe { library.as_ref() }.ok_or(INVALID)?;
                if !ptr::eq(e.device, d) {
                    return Err(INVALID);
                }
                let meta = e.ray_meta.as_ref().ok_or(INVALID)?;
                compatible(meta, state, info)?;
                append(&mut result.stages, &meta.stages)?;
                append(&mut result.imported_groups, &meta.groups)?;
                result.libraries.push(e.pipeline);
            }
            result.interface=Some(vk::VkRayTracingPipelineInterfaceCreateInfoKHR {
                sType:vk::VkStructureType_VK_STRUCTURE_TYPE_RAY_TRACING_PIPELINE_INTERFACE_CREATE_INFO_KHR,
                maxPipelineRayPayloadSize:info.max_payload_size,maxPipelineRayHitAttributeSize:info.max_hit_attribute_size,..Default::default()
            });
        }
        if shaders.is_empty() && result.libraries.is_empty() {
            return Err(INVALID);
        }
        if !library && !result.stages.contains(&SHADER_RAYGEN) {
            return Err(INVALID);
        }
        if let Some(info) = replay {
            if info.reserved != 0 || info.count != state.group_count {
                return Err(INVALID);
            }
            let handles = unsafe { commands::slice(info.handles, info.count)? };
            result
                .replay
                .try_reserve_exact(handles.len())
                .map_err(|_| OUT_OF_MEMORY)?;
            for h in handles {
                let p = if h.size == 0 {
                    ptr::null()
                } else {
                    if state.flags & RAY_CAPTURE_HANDLES == 0 {
                        return Err(INVALID);
                    }
                    if h.data.is_null()
                        || h.size != d.snapshot.ray_artifact_limits.capture_handle_size as usize
                    {
                        return Err(INVALID);
                    }
                    h.data
                };
                result.replay.push(p);
            }
        }
        Ok(result)
    }
    pub fn metadata(
        &self,
        state: &RayState,
        local_groups: &[u8],
        library: bool,
    ) -> Result<Box<ray::PipelineMeta>, Status> {
        let mut groups = Vec::new();
        append(&mut groups, local_groups)?;
        append(&mut groups, &self.imported_groups)?;
        if !library && groups.is_empty() {
            return Err(INVALID);
        }
        let mut stages = Vec::new();
        append(&mut stages, &self.stages)?;
        Ok(Box::new(ray::PipelineMeta {
            library,
            stages: stages.into_boxed_slice(),
            groups: groups.into_boxed_slice(),
            flags: state.flags,
            max_recursion: state.max_recursion,
            max_payload: self
                .interface
                .as_ref()
                .map_or(0, |i| i.maxPipelineRayPayloadSize),
            max_attribute: self
                .interface
                .as_ref()
                .map_or(0, |i| i.maxPipelineRayHitAttributeSize),
        }))
    }
}
fn append<T: Copy>(out: &mut Vec<T>, values: &[T]) -> Result<(), Status> {
    if out.len().checked_add(values.len()).ok_or(UNSUPPORTED)? > u32::MAX as usize {
        return Err(UNSUPPORTED);
    }
    out.try_reserve_exact(values.len())
        .map_err(|_| OUT_OF_MEMORY)?;
    out.extend_from_slice(values);
    Ok(())
}
fn compatible(
    meta: &ray::PipelineMeta,
    state: &RayState,
    info: &RayLibrariesInfo,
) -> Result<(), Status> {
    if !meta.library
        || meta.max_recursion != state.max_recursion
        || meta.max_payload != info.max_payload_size
        || meta.max_attribute != info.max_hit_attribute_size
        || state.flags & 127 & !meta.flags != 0
    {
        return Err(INVALID);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn linkage_preserves_library_flags_interfaces_and_order() {
        let meta = ray::PipelineMeta {
            library: true,
            stages: Box::new([SHADER_MISS, SHADER_CLOSEST_HIT]),
            groups: Box::new([1, 2]),
            flags: RAY_CAPTURE_HANDLES | RAY_NO_NULL_CLOSEST_HIT,
            max_recursion: 2,
            max_payload: 24,
            max_attribute: 8,
        };
        let state = RayState {
            header: Record::new::<RayState>(RAY_STATE),
            group_count: 1,
            max_recursion: 2,
            groups: ptr::null(),
            flags: RAY_CAPTURE_HANDLES,
            reserved: 0,
        };
        let info = RayLibrariesInfo {
            header: Record::new::<RayLibrariesInfo>(RAY_LIBRARIES_INFO),
            count: 0,
            max_payload_size: 24,
            max_hit_attribute_size: 8,
            reserved: 0,
            libraries: ptr::null(),
        };
        assert_eq!(compatible(&meta, &state, &info), Ok(()));
        assert_eq!(
            compatible(
                &meta,
                &RayState {
                    flags: RAY_NO_NULL_ANY_HIT,
                    ..state
                },
                &info
            ),
            Err(INVALID)
        );
        assert_eq!(
            compatible(
                &meta,
                &RayState {
                    max_recursion: 1,
                    ..state
                },
                &info
            ),
            Err(INVALID)
        );
        assert_eq!(
            compatible(
                &meta,
                &state,
                &RayLibrariesInfo {
                    max_payload_size: 32,
                    ..info
                }
            ),
            Err(INVALID)
        );
        let link = Linkage {
            libraries: Vec::new(),
            interface: None,
            stages: vec![SHADER_RAYGEN, SHADER_MISS, SHADER_CLOSEST_HIT],
            imported_groups: vec![1, 2],
            replay: Vec::new(),
        };
        let m = link.metadata(&state, &[1, 8], false).unwrap();
        assert_eq!(&*m.groups, &[1, 8, 1, 2]);
        assert_eq!(
            &*m.stages,
            &[SHADER_RAYGEN, SHADER_MISS, SHADER_CLOSEST_HIT]
        );
    }
}
