//! Explicit sparse storage and queue binding; no residency registry or migration.
use super::*;
pub(super) fn available(f: &vk::VkPhysicalDeviceFeatures) -> FeatureBits {
    let mut result = FeatureBits::ZERO;
    for (bit, value) in [
        (SPARSE_BINDING, f.sparseBinding),
        (SPARSE_BUFFER, f.sparseResidencyBuffer),
        (SPARSE_IMAGE_2D, f.sparseResidencyImage2D),
        (SPARSE_IMAGE_3D, f.sparseResidencyImage3D),
        (SPARSE_IMAGE_2_SAMPLES, f.sparseResidency2Samples),
        (SPARSE_IMAGE_4_SAMPLES, f.sparseResidency4Samples),
        (SPARSE_IMAGE_8_SAMPLES, f.sparseResidency8Samples),
        (SPARSE_IMAGE_16_SAMPLES, f.sparseResidency16Samples),
        (SPARSE_ALIAS, f.sparseResidencyAliased),
        (SHADER_SPARSE_RESIDENCY, f.shaderResourceResidency),
        (SHADER_MIN_LOD, f.shaderResourceMinLod),
    ] {
        if value != 0 {
            result |= bit;
        }
    }
    result
}
pub(super) fn limits(p: &vk::VkPhysicalDeviceProperties) -> SparseLimits {
    let s = &p.sparseProperties;
    SparseLimits {
        address_space_size: p.limits.sparseAddressSpaceSize,
        standard_2d: s.residencyStandard2DBlockShape,
        standard_2d_multisample: s.residencyStandard2DMultisampleBlockShape,
        standard_3d: s.residencyStandard3DBlockShape,
        aligned_mip_size: s.residencyAlignedMipSize,
        nonresident_strict: s.residencyNonResidentStrict,
        descriptor_heaps: 0,
    }
}
pub(super) fn image_features(
    d: &Device,
    flags: u32,
    dimension: u32,
    samples: u32,
) -> Result<(), Status> {
    if flags & (IMAGE_SPARSE_RESIDENCY | IMAGE_SPARSE_ALIAS) != 0 && flags & IMAGE_SPARSE == 0 {
        return Err(INVALID);
    }
    let mut required = FeatureBits::ZERO;
    if flags & IMAGE_SPARSE != 0 {
        required |= SPARSE_BINDING;
    }
    if flags & IMAGE_SPARSE_ALIAS != 0 {
        required |= SPARSE_ALIAS;
    }
    if flags & IMAGE_SPARSE_RESIDENCY != 0 {
        required |= match dimension {
            2 => SPARSE_IMAGE_2D,
            3 => SPARSE_IMAGE_3D,
            _ => return Err(UNSUPPORTED),
        };
        required |= match samples {
            1 => FeatureBits::ZERO,
            2 => SPARSE_IMAGE_2_SAMPLES,
            4 => SPARSE_IMAGE_4_SAMPLES,
            8 => SPARSE_IMAGE_8_SAMPLES,
            16 => SPARSE_IMAGE_16_SAMPLES,
            _ => return Err(UNSUPPORTED),
        };
    }
    if required & !d.snapshot.features.enabled != 0 {
        return Err(UNSUPPORTED);
    }
    Ok(())
}

use super::commands::{scratch, slice};
use std::alloc::Layout;

// The same traversal sizes and fills caller storage. Planning never dereferences
// GPU objects; translation validates every entry before the single native call.
struct Storage {
    layout: Layout,
    base: *mut u8,
}
impl Storage {
    fn new(base: *mut u8) -> Self {
        Self {
            layout: Layout::new::<()>(),
            base,
        }
    }
    fn array<T>(&mut self, count: usize) -> Result<*mut T, Status> {
        let array = Layout::array::<T>(count).map_err(|_| INVALID)?;
        let (layout, offset) = self.layout.extend(array).map_err(|_| INVALID)?;
        self.layout = layout;
        Ok(self.base.wrapping_add(offset).cast())
    }
    fn requirements(&self) -> HostRequirements {
        let layout = self.layout.pad_to_align();
        HostRequirements {
            size: layout.size() as u64,
            alignment: layout.align() as u64,
        }
    }
}
unsafe fn walk(
    descs: &[SparseSubmit],
    s: &mut Storage,
    device: Option<&Device>,
) -> Result<*mut vk::VkBindSparseInfo, Status> {
    u32::try_from(descs.len()).map_err(|_| INVALID)?;
    let infos = s.array::<vk::VkBindSparseInfo>(descs.len())?;
    let timelines = s.array::<vk::VkTimelineSemaphoreSubmitInfo>(descs.len())?;
    for (i, desc) in descs.iter().enumerate() {
        if desc.flags != 0 {
            return Err(UNSUPPORTED);
        }
        let buffers = unsafe { slice(desc.buffers, desc.buffer_count)? };
        let opaque = unsafe { slice(desc.opaque_images, desc.opaque_image_count)? };
        let images = unsafe { slice(desc.images, desc.image_count)? };
        let nb = s.array::<vk::VkSparseBufferMemoryBindInfo>(buffers.len())?;
        let no = s.array::<vk::VkSparseImageOpaqueMemoryBindInfo>(opaque.len())?;
        let ni = s.array::<vk::VkSparseImageMemoryBindInfo>(images.len())?;
        for (j, group) in buffers.iter().enumerate() {
            if group.reserved != 0 || group.count == 0 {
                return Err(INVALID);
            }
            let binds = unsafe { slice(group.binds, group.count)? };
            let native = s.array::<vk::VkSparseMemoryBind>(binds.len())?;
            if let Some(d) = device {
                let memory = unsafe { group.memory.as_ref() }.ok_or(INVALID)?;
                let buffer = memory.sparse_handle(d)?;
                for (k, bind) in binds.iter().enumerate() {
                    unsafe {
                        native.add(k).write(memory.sparse_bind(d, bind)?);
                    }
                }
                unsafe {
                    nb.add(j).write(vk::VkSparseBufferMemoryBindInfo {
                        buffer,
                        bindCount: group.count,
                        pBinds: native,
                    });
                }
            }
        }
        for (j, group) in opaque.iter().enumerate() {
            if group.reserved != 0 || group.count == 0 {
                return Err(INVALID);
            }
            let binds = unsafe { slice(group.binds, group.count)? };
            let native = s.array::<vk::VkSparseMemoryBind>(binds.len())?;
            if let Some(d) = device {
                let image = unsafe { group.image.as_ref() }.ok_or(INVALID)?;
                let handle = image.sparse_handle(d)?;
                for (k, bind) in binds.iter().enumerate() {
                    unsafe {
                        native.add(k).write(image.sparse_opaque_bind(d, bind)?);
                    }
                }
                unsafe {
                    no.add(j).write(vk::VkSparseImageOpaqueMemoryBindInfo {
                        image: handle,
                        bindCount: group.count,
                        pBinds: native,
                    });
                }
            }
        }
        for (j, group) in images.iter().enumerate() {
            if group.reserved != 0 || group.count == 0 {
                return Err(INVALID);
            }
            let binds = unsafe { slice(group.binds, group.count)? };
            let native = s.array::<vk::VkSparseImageMemoryBind>(binds.len())?;
            if let Some(d) = device {
                let image = unsafe { group.image.as_ref() }.ok_or(INVALID)?;
                let handle = image.sparse_handle(d)?;
                for (k, bind) in binds.iter().enumerate() {
                    unsafe {
                        native.add(k).write(image.sparse_image_bind(d, bind)?);
                    }
                }
                unsafe {
                    ni.add(j).write(vk::VkSparseImageMemoryBindInfo {
                        image: handle,
                        bindCount: group.count,
                        pBinds: native,
                    });
                }
            }
        }
        let mut semaphores = [ptr::null_mut(); 2];
        let mut values = [ptr::null_mut(); 2];
        let mut counts = [0; 2];
        for (j, (points, pc, binaries, bc)) in [
            (
                desc.waits,
                desc.wait_count,
                desc.binary_waits,
                desc.binary_wait_count,
            ),
            (
                desc.signals,
                desc.signal_count,
                desc.binary_signals,
                desc.binary_signal_count,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            counts[j] = pc.checked_add(bc).ok_or(INVALID)?;
            let points = unsafe { slice(points, pc)? };
            let binaries = unsafe { slice(binaries, bc)? };
            let sem = s.array::<vk::VkSemaphore>(counts[j] as usize)?;
            let val = s.array::<u64>(counts[j] as usize)?;
            semaphores[j] = sem;
            values[j] = val;
            if let Some(d) = device {
                for (k, point) in points.iter().enumerate() {
                    let t = unsafe { point.timeline.as_ref() }.ok_or(INVALID)?;
                    if !ptr::eq(t.device, d) {
                        return Err(INVALID);
                    }
                    unsafe {
                        sem.add(k).write(t.handle);
                        val.add(k).write(point.value);
                    }
                }
                for (k, binary) in binaries.iter().enumerate() {
                    let b = unsafe { binary.as_ref() }.ok_or(INVALID)?;
                    if !ptr::eq(b.device, d) {
                        return Err(INVALID);
                    }
                    unsafe {
                        sem.add(points.len() + k).write(b.handle);
                        val.add(points.len() + k).write(0);
                    }
                }
            }
        }
        if device.is_some() {
            unsafe {
                timelines.add(i).write(vk::VkTimelineSemaphoreSubmitInfo {
                    sType: vk::VkStructureType_VK_STRUCTURE_TYPE_TIMELINE_SEMAPHORE_SUBMIT_INFO,
                    waitSemaphoreValueCount: counts[0],
                    pWaitSemaphoreValues: values[0],
                    signalSemaphoreValueCount: counts[1],
                    pSignalSemaphoreValues: values[1],
                    ..Default::default()
                });
                infos.add(i).write(vk::VkBindSparseInfo {
                    sType: vk::VkStructureType_VK_STRUCTURE_TYPE_BIND_SPARSE_INFO,
                    pNext: timelines.add(i).cast(),
                    waitSemaphoreCount: counts[0],
                    pWaitSemaphores: semaphores[0],
                    signalSemaphoreCount: counts[1],
                    pSignalSemaphores: semaphores[1],
                    bufferBindCount: desc.buffer_count,
                    pBufferBinds: nb,
                    imageOpaqueBindCount: desc.opaque_image_count,
                    pImageOpaqueBinds: no,
                    imageBindCount: desc.image_count,
                    pImageBinds: ni,
                });
            }
        }
    }
    Ok(infos)
}
pub(in crate::foundation) unsafe fn requirements(
    d: &Device,
    descs: &[SparseSubmit],
) -> Result<HostRequirements, Status> {
    d.ready()?;
    let mut storage = Storage::new(ptr::null_mut());
    unsafe {
        walk(descs, &mut storage, None)?;
    }
    Ok(storage.requirements())
}
impl Queue {
    pub(in crate::foundation) unsafe fn bind_sparse_to(
        &self,
        descs: &[SparseSubmit],
        outcomes: *mut SubmitOutcome,
        outcome_count: usize,
        storage: HostSpan,
    ) -> Result<(), Status> {
        let submit = || -> Result<SubmitOutcome, Status> {
            if outcome_count != descs.len() {
                return Err(INVALID);
            }
            let d = unsafe { self.device.load(Ordering::Relaxed).as_ref() }.ok_or(INVALID)?;
            d.ready()?;
            if d.snapshot.features.enabled & SPARSE_BINDING == 0 {
                return Err(UNSUPPORTED);
            }
            if !d
                .snapshot
                .queues
                .iter()
                .any(|q| q.domain == self.domain && q.flags & QUEUE_SPARSE != 0)
            {
                return Err(UNSUPPORTED);
            }
            if descs.is_empty() {
                return Ok(SubmitOutcome {
                    status: OK,
                    acceptance: SUBMIT_ACCEPTED,
                });
            }
            let required = unsafe { requirements(d, descs)? };
            let base = scratch(storage.data, storage.size, required)?;
            let mut arena = Storage::new(base);
            let infos = unsafe { walk(descs, &mut arena, Some(d))? };
            let native = unsafe {
                (d.f.vkQueueBindSparse.unwrap())(
                    self.handle,
                    descs.len() as u32,
                    infos,
                    ptr::null_mut(),
                )
            };
            let outcome = classify(native);
            if outcome.acceptance == SUBMIT_UNKNOWN {
                d.lost.store(true, Ordering::Relaxed);
            }
            Ok(outcome)
        };
        let outcome = submit().unwrap_or_else(SubmitOutcome::rejected);
        for i in 0..outcome_count {
            unsafe {
                outcomes.add(i).write(outcome);
            }
        }
        if outcome.status == OK {
            Ok(())
        } else {
            Err(outcome.status)
        }
    }
}
fn classify(native: vk::VkResult) -> SubmitOutcome {
    // No queue-submit OOM rollback guarantee is assumed for sparse binding.
    if native == vk::VkResult_VK_SUCCESS {
        SubmitOutcome {
            status: OK,
            acceptance: SUBMIT_ACCEPTED,
        }
    } else {
        SubmitOutcome {
            status: status(native).unwrap_err(),
            acceptance: SUBMIT_UNKNOWN,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sparse_failure_does_not_borrow_submit_rollback_guarantee() {
        assert_eq!(
            classify(vk::VkResult_VK_SUCCESS).acceptance,
            SUBMIT_ACCEPTED
        );
        for (native, expected) in [
            (vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY, OUT_OF_MEMORY),
            (vk::VkResult_VK_ERROR_OUT_OF_DEVICE_MEMORY, OUT_OF_MEMORY),
            (vk::VkResult_VK_ERROR_DEVICE_LOST, DEVICE_LOST),
            (vk::VkResult_VK_ERROR_UNKNOWN, BACKEND_ERROR),
        ] {
            assert_eq!(
                classify(native),
                SubmitOutcome {
                    status: expected,
                    acceptance: SUBMIT_UNKNOWN
                }
            );
        }
    }
    #[test]
    fn sparse_nested_scratch_checks_extents_without_resource_access() {
        let mut storage = Storage::new(ptr::null_mut());
        unsafe {
            walk(&[], &mut storage, None).unwrap();
        }
        assert_eq!(storage.requirements().size, 0);
        let binding = SparseMemoryBind {
            offset: 0,
            size: 1,
            physical: Span {
                memory: ptr::dangling_mut(),
                offset: 0,
                size: 1,
            },
            flags: 0,
            reserved: 0,
        };
        let group = SparseBufferBinds {
            memory: ptr::dangling_mut(),
            count: 1,
            reserved: 0,
            binds: &binding,
        };
        let point = Point {
            timeline: ptr::dangling_mut(),
            value: 1,
        };
        let desc = SparseSubmit {
            buffer_count: 1,
            buffers: &group,
            signal_count: 1,
            signals: &point,
            ..Default::default()
        };
        let mut storage = Storage::new(ptr::null_mut());
        unsafe {
            walk(&[desc, desc], &mut storage, None).unwrap();
        }
        let per_batch = size_of::<vk::VkBindSparseInfo>()
            + size_of::<vk::VkTimelineSemaphoreSubmitInfo>()
            + size_of::<vk::VkSparseBufferMemoryBindInfo>()
            + size_of::<vk::VkSparseMemoryBind>()
            + size_of::<vk::VkSemaphore>()
            + size_of::<u64>();
        assert!(storage.requirements().size >= 2 * per_batch as u64);
        for bad in [
            SparseSubmit {
                buffers: ptr::null(),
                ..desc
            },
            SparseSubmit {
                signal_count: u32::MAX,
                binary_signal_count: 1,
                ..desc
            },
        ] {
            assert!(matches!(
                unsafe { walk(&[desc, bad], &mut Storage::new(ptr::null_mut()), None) },
                Err(INVALID)
            ));
        }
        assert!(Storage::new(ptr::null_mut())
            .array::<vk::VkBindSparseInfo>(usize::MAX)
            .is_err());
    }
    #[test]
    fn sparse_features_keep_independent_residency_and_shader_contracts() {
        for child in [
            SPARSE_BUFFER,
            SPARSE_IMAGE_2D,
            SPARSE_IMAGE_3D,
            SPARSE_ALIAS,
            SPARSE_IMAGE_2_SAMPLES,
            SPARSE_IMAGE_16_SAMPLES,
        ] {
            assert_eq!(validate_feature_dependencies(child), Err(INVALID));
            assert_eq!(
                validate_feature_dependencies(child | SPARSE_BINDING),
                Ok(())
            );
        }
        assert_eq!(
            validate_feature_dependencies(SHADER_SPARSE_RESIDENCY | SHADER_MIN_LOD),
            Ok(())
        );
        let mut f = vk::VkPhysicalDeviceFeatures {
            sparseBinding: 1,
            ..Default::default()
        };
        assert_eq!(available(&f), SPARSE_BINDING);
        f.sparseResidencyImage3D = 1;
        assert_eq!(available(&f), SPARSE_BINDING | SPARSE_IMAGE_3D);
    }
}
