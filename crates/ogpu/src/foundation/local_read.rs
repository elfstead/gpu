//! Native local-read mappings. Translation borrows caller arrays; no descriptor
//! rewriting, attachment retention, shader reflection or automatic dependencies.
use super::commands::slice;
use super::*;
use std::ffi::c_void;

/// Validate the parent record while admitting only the separately parsed mapping.
/// The caller must parse and validate the admitted next chain before native use.
pub(super) unsafe fn parent<'a, T>(p: *const Record, kind: u32) -> Result<&'a T, Status> {
    let header = unsafe { p.as_ref() }.ok_or(INVALID)?;
    Record {
        next: ptr::null(),
        ..*header
    }
    .validate::<T>(kind)?;
    Ok(unsafe { &*p.cast() })
}
fn count(d: &Device, count: u32, expected: u32, reserved: u32) -> Result<(), Status> {
    if d.snapshot.features.enabled & LOCAL_READ == 0 {
        return Err(UNSUPPORTED);
    }
    if reserved != 0 || count != expected {
        return Err(INVALID);
    }
    if count > d.snapshot.graphics_limits.max_colors {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
unsafe fn indices<'a>(p: *const u32, count: u32) -> Result<Option<&'a [u32]>, Status> {
    if p.is_null() {
        Ok(None)
    } else {
        Ok(Some(unsafe { slice(p, count)? }))
    }
}
fn validate_indices(
    values: Option<&[u32]>,
    count: u32,
    maximum: u32,
    depth: Option<u32>,
    stencil: Option<u32>,
) -> Result<(), Status> {
    let get = |i: usize| values.map_or(i as u32, |v| v[i]);
    for i in 0..count as usize {
        let v = get(i);
        if v == ATTACHMENT_UNUSED {
            continue;
        }
        if v >= maximum {
            return Err(UNSUPPORTED);
        }
        if depth == Some(v) || stencil == Some(v) || (0..i).any(|j| get(j) == v) {
            return Err(INVALID);
        }
    }
    for value in [depth, stencil].into_iter().flatten() {
        if value != ATTACHMENT_UNUSED && value >= maximum {
            return Err(UNSUPPORTED);
        }
    }
    // Depth and stencil may intentionally share an input index.
    Ok(())
}
pub(super) unsafe fn locations(
    d: &Device,
    s: &RenderLocations,
    expected: u32,
) -> Result<vk::VkRenderingAttachmentLocationInfo, Status> {
    count(d, s.color_count, expected, s.reserved)?;
    validate_indices(
        unsafe { indices(s.locations, s.color_count)? },
        s.color_count,
        d.snapshot.graphics_limits.max_colors,
        None,
        None,
    )?;
    Ok(vk::VkRenderingAttachmentLocationInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_RENDERING_ATTACHMENT_LOCATION_INFO,
        colorAttachmentCount: s.color_count,
        pColorAttachmentLocations: s.locations,
        ..Default::default()
    })
}
pub(super) unsafe fn inputs(
    d: &Device,
    s: &RenderInputs,
    expected: u32,
) -> Result<vk::VkRenderingInputAttachmentIndexInfo, Status> {
    count(d, s.color_count, expected, s.reserved)?;
    validate_indices(
        unsafe { indices(s.colors, s.color_count)? },
        s.color_count,
        d.snapshot.local_read_limits.max_inputs,
        unsafe { s.depth.as_ref().copied() },
        unsafe { s.stencil.as_ref().copied() },
    )?;
    Ok(vk::VkRenderingInputAttachmentIndexInfo {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_RENDERING_INPUT_ATTACHMENT_INDEX_INFO,
        colorAttachmentCount: s.color_count,
        pColorAttachmentInputIndices: s.colors,
        pDepthInputAttachmentIndex: s.depth,
        pStencilInputAttachmentIndex: s.stencil,
        ..Default::default()
    })
}
#[derive(Default)]
pub(super) struct PreparedMapping {
    pub bindings: Vec<vk::VkDescriptorSetAndBindingMappingEXT>,
    locations: Option<vk::VkRenderingAttachmentLocationInfo>,
    inputs: Option<vk::VkRenderingInputAttachmentIndexInfo>,
}
impl PreparedMapping {
    pub fn link(&mut self, tail: *const c_void) -> *const c_void {
        let mut head = tail;
        if let Some(v) = self.inputs.as_mut() {
            v.pNext = head;
            head = ptr::from_ref(v).cast();
        }
        if let Some(v) = self.locations.as_mut() {
            v.pNext = head;
            head = ptr::from_ref(v).cast();
        }
        head
    }
}
pub(super) unsafe fn prepare(
    d: &Device,
    p: *const Record,
    colors: u32,
    arguments: Option<&ArgumentLayout>,
) -> Result<PreparedMapping, Status> {
    if p.is_null() {
        return Ok(PreparedMapping::default());
    }
    let m = unsafe { executable_record::<RenderMapping>(p, RENDER_MAPPING)? };
    if d.snapshot.features.enabled & LOCAL_READ == 0 {
        return Err(UNSUPPORTED);
    }
    if m.reserved != 0 {
        return Err(INVALID);
    }
    let mut bindings = Vec::new();
    if m.binding_count != 0 {
        let arguments = arguments.ok_or(INVALID)?; // resource bindings belong to executable preparation
        let source = unsafe { slice(m.bindings, m.binding_count)? };
        bindings
            .try_reserve_exact(source.len())
            .map_err(|_| OUT_OF_MEMORY)?;
        for (i, b) in source.iter().enumerate() {
            if b.root_offset % 4 != 0
                || u64::from(b.root_offset) + 4 > u64::from(arguments.byte_size())
                || u64::from(b.array_stride) % d.snapshot.descriptor_properties.image_alignment != 0
                || source[..i]
                    .iter()
                    .any(|old| old.space == b.space && old.binding == b.binding)
            {
                return Err(INVALID);
            }
            bindings.push(vk::VkDescriptorSetAndBindingMappingEXT {
                sType:vk::VkStructureType_VK_STRUCTURE_TYPE_DESCRIPTOR_SET_AND_BINDING_MAPPING_EXT,
                descriptorSet:b.space,firstBinding:b.binding,bindingCount:1,
                // SubpassData is read-only semantically, but compilers need not emit
                // NonWritable. Native resource classification uses that decoration.
                resourceMask:vk::VkSpirvResourceTypeFlagBitsEXT_VK_SPIRV_RESOURCE_TYPE_READ_ONLY_IMAGE_BIT_EXT
                    | vk::VkSpirvResourceTypeFlagBitsEXT_VK_SPIRV_RESOURCE_TYPE_READ_WRITE_IMAGE_BIT_EXT,
                source:vk::VkDescriptorMappingSourceEXT_VK_DESCRIPTOR_MAPPING_SOURCE_HEAP_WITH_PUSH_INDEX_EXT,
                sourceData:vk::VkDescriptorMappingSourceDataEXT { pushIndex:vk::VkDescriptorMappingSourcePushIndexEXT {
                    pushOffset:b.root_offset,heapIndexStride:1,heapArrayStride:b.array_stride,..Default::default()
                } },..Default::default()
            });
        }
        let ordinary = arguments.mapping_info(16, ptr::null())?;
        bindings
            .try_reserve_exact(ordinary.mappingCount as usize)
            .map_err(|_| OUT_OF_MEMORY)?;
        bindings.extend_from_slice(unsafe { slice(ordinary.pMappings, ordinary.mappingCount)? });
    }
    Ok(PreparedMapping {
        bindings,
        locations: unsafe { m.locations.as_ref() }
            .map(|v| unsafe { locations(d, v, colors) })
            .transpose()?,
        inputs: unsafe { m.inputs.as_ref() }
            .map(|v| unsafe { inputs(d, v, colors) })
            .transpose()?,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mappings_preserve_native_defaults_unused_and_depth_stencil_aliasing() {
        assert_eq!(validate_indices(None, 2, 4, Some(2), Some(2)), Ok(()));
        assert_eq!(validate_indices(None, 2, 4, Some(1), None), Err(INVALID));
        assert_eq!(
            validate_indices(Some(&[ATTACHMENT_UNUSED, 0]), 2, 4, Some(1), Some(1)),
            Ok(())
        );
        assert_eq!(
            validate_indices(Some(&[1, 1]), 2, 4, None, None),
            Err(INVALID)
        );
        assert_eq!(
            validate_indices(Some(&[4]), 1, 4, None, None),
            Err(UNSUPPORTED)
        );
        assert_eq!(
            validate_indices(None, 0, 4, Some(ATTACHMENT_UNUSED), Some(4)),
            Err(UNSUPPORTED)
        );
        assert_eq!(
            validate_indices(Some(&[ATTACHMENT_UNUSED; 2]), 2, 4, None, None),
            Ok(())
        );
    }
    #[test]
    fn mapping_chain_relinks_after_move() {
        let mut p = PreparedMapping {
            locations: Some(Default::default()),
            inputs: Some(Default::default()),
            ..Default::default()
        };
        let tail = ptr::dangling::<u32>().cast();
        assert_eq!(
            p.link(tail),
            ptr::from_ref(p.locations.as_ref().unwrap()).cast()
        );
        let mut moved = p;
        moved.link(tail);
        assert_eq!(
            moved.locations.as_ref().unwrap().pNext,
            ptr::from_ref(moved.inputs.as_ref().unwrap()).cast()
        );
        assert_eq!(moved.inputs.as_ref().unwrap().pNext, tail);
    }
    #[test]
    #[ignore = "requires modern Vulkan; independent local-read feature enable/rejection"]
    fn gpu_local_read_feature_contract() {
        let instance = Arc::new(Instance::new().unwrap());
        let mut checked = false;
        for physical in instance.physical_devices().unwrap() {
            let snapshot = super::super::snapshot(&instance, physical).unwrap();
            if snapshot.features.baseline_supported == 0 {
                continue;
            }
            let Some(q) = snapshot
                .queues
                .iter()
                .find(|q| q.flags & GRAPHICS != 0 && q.count != 0)
            else {
                continue;
            };
            let request = [QueueRequest {
                domain: q.domain,
                count: 1,
                priority: 0.5,
            }];
            let available = snapshot.features.available;
            let adapter = Adapter {
                instance: instance.clone(),
                physical,
                snapshot,
            };
            let disabled = Device::create(&adapter, &request, RASTER).unwrap();
            let l = RenderLocations {
                color_count: 2,
                reserved: 0,
                locations: ptr::null(),
            };
            assert!(matches!(
                unsafe { locations(&disabled, &l, 2) },
                Err(UNSUPPORTED)
            ));
            let m = RenderMapping {
                header: Record::new::<RenderMapping>(RENDER_MAPPING),
                locations: ptr::null(),
                inputs: ptr::null(),
                binding_count: 0,
                reserved: 0,
                bindings: ptr::null(),
            };
            assert!(matches!(
                unsafe { prepare(&disabled, &m.header, 2, None) },
                Err(UNSUPPORTED)
            ));
            let result = Device::create(&adapter, &request, RASTER | LOCAL_READ);
            if available & LOCAL_READ == 0 {
                assert!(matches!(result, Err(UNSUPPORTED)));
            } else {
                let enabled = result.unwrap();
                assert_eq!(enabled.snapshot.features.enabled, RASTER | LOCAL_READ);
                assert_eq!(
                    enabled.snapshot.local_read_limits,
                    disabled.snapshot.local_read_limits
                );
                assert!(unsafe { locations(&enabled, &l, 2) }.is_ok());
                assert!(matches!(
                    unsafe { locations(&enabled, &l, 1) },
                    Err(INVALID)
                ));
                // Inheritance accepts mappings, never executable resource bindings.
                let bad = RenderMapping {
                    binding_count: 1,
                    ..m
                };
                assert!(matches!(
                    unsafe { prepare(&enabled, &bad.header, 2, None) },
                    Err(INVALID)
                ));
            }
            checked = true;
        }
        assert!(checked, "no compatible physical device");
    }
}
