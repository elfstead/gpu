//! Acceleration recording: validate into caller scratch, then one native batch.
use super::*;
impl List {
    fn acceleration_device(&self, copy: bool) -> Result<&Device, Status> {
        let d = self.outside_render()?;
        acceleration::enabled(d)?;
        let scope = if copy && d.snapshot.features.enabled & RAY_MAINTENANCE != 0 {
            ACCELERATION_COPY_STAGE
        } else {
            ACCELERATION_BUILD_STAGE
        };
        stages(d, self.domain, scope, false)?;
        Ok(d)
    }
    pub(in crate::foundation) unsafe fn build_accelerations(
        &self,
        work: &[AccelerationBuild],
        scratch: HostSpan,
    ) -> Result<(), Status> {
        let d = self.acceleration_device(false)?;
        let count = u32::try_from(work.len()).map_err(|_| INVALID)?;
        if count == 0 {
            return Ok(());
        }
        let indirect = work[0].indirect.address != 0;
        if indirect && d.snapshot.features.enabled & ACCELERATION_INDIRECT_BUILD == 0 {
            return Err(UNSUPPORTED);
        }
        let mut geometry_count = 0u32;
        for w in work {
            let desc = unsafe { w.description.as_ref() }.ok_or(INVALID)?;
            desc.header
                .validate::<AccelerationBuildDesc>(ACCELERATION_BUILD_DESC)?;
            geometry_count = geometry_count
                .checked_add(desc.geometry_count)
                .ok_or(INVALID)?;
        }
        let (layout, offsets) = acceleration::layout(count, geometry_count)?;
        let base = acceleration::workspace(scratch, layout)?;
        let infos = base.cast::<vk::VkAccelerationStructureBuildGeometryInfoKHR>();
        let mut geometry = unsafe {
            base.add(offsets[0])
                .cast::<vk::VkAccelerationStructureGeometryKHR>()
        };
        let ranges = unsafe {
            base.add(offsets[1])
                .cast::<*const vk::VkAccelerationStructureBuildRangeInfoKHR>()
        };
        let addresses = unsafe { base.add(offsets[2]).cast::<u64>() };
        let strides = unsafe { base.add(offsets[3]).cast::<u32>() };
        let maxima = unsafe { base.add(offsets[4]).cast::<*const u32>() };
        for (i, w) in work.iter().enumerate() {
            let desc = unsafe { &*w.description };
            let dst = unsafe { w.destination.as_ref() }.ok_or(INVALID)?;
            dst.check(d, self.domain)?;
            if w.update > 1
                || dst.kind != 3 && dst.kind != desc.kind
                || indirect != (w.indirect.address != 0)
            {
                return Err(INVALID);
            }
            if (w.scratch.size != 0 && w.scratch.address == 0)
                || w.scratch.address % d.snapshot.ray_limits.scratch_alignment != 0
            {
                return Err(INVALID);
            }
            w.scratch
                .address
                .checked_add(w.scratch.size)
                .ok_or(INVALID)?;
            let mut info = unsafe { acceleration::prepare(d, desc, geometry)? };
            info.dstAccelerationStructure = dst.handle;
            info.mode = w.update;
            info.scratchData = vk::VkDeviceOrHostAddressKHR {
                deviceAddress: w.scratch.address,
            };
            if w.update != 0 {
                let src = unsafe { w.source.as_ref() }.ok_or(INVALID)?;
                src.check(d, self.domain)?;
                if desc.flags & 1 == 0 || src.kind != 3 && src.kind != desc.kind {
                    return Err(INVALID);
                }
                info.srcAccelerationStructure = src.handle;
            } else if !w.source.is_null() {
                return Err(INVALID);
            }
            if indirect {
                if !w.ranges.is_null()
                    || w.indirect.address % 4 != 0
                    || w.indirect_stride < 16
                    || w.indirect_stride % 4 != 0
                {
                    return Err(INVALID);
                }
                let bytes = if desc.geometry_count == 0 {
                    0
                } else {
                    u64::from(desc.geometry_count - 1)
                        .checked_mul(u64::from(w.indirect_stride))
                        .and_then(|n| n.checked_add(16))
                        .ok_or(INVALID)?
                };
                if w.indirect.size < bytes {
                    return Err(INVALID);
                }
                w.indirect
                    .address
                    .checked_add(w.indirect.size)
                    .ok_or(INVALID)?;
            } else {
                if w.indirect.size != 0 || w.indirect_stride != 0 {
                    return Err(INVALID);
                }
                let r = unsafe { slice(w.ranges, desc.geometry_count)? };
                let max = unsafe { slice(desc.max_primitives, desc.geometry_count)? };
                for (r, max) in r.iter().zip(max) {
                    if r.primitive_count > *max || r.transform_offset % 16 != 0 {
                        return Err(INVALID);
                    }
                }
            }
            unsafe {
                infos.add(i).write(info);
                ranges.add(i).write(w.ranges.cast());
                addresses.add(i).write(w.indirect.address);
                strides.add(i).write(w.indirect_stride);
                maxima.add(i).write(desc.max_primitives);
                geometry = geometry.add(desc.geometry_count as usize);
            }
        }
        unsafe {
            if indirect {
                (d.acceleration_commands.indirect.ok_or(UNSUPPORTED)?)(
                    self.command,
                    count,
                    infos,
                    addresses,
                    strides,
                    maxima,
                );
            } else {
                (d.acceleration_commands.build.ok_or(UNSUPPORTED)?)(
                    self.command,
                    count,
                    infos,
                    ranges,
                );
            }
        }
        Ok(())
    }
    pub(in crate::foundation) fn copy_acceleration(
        &self,
        dst: &Acceleration,
        src: &Acceleration,
        mode: u32,
    ) -> Result<(), Status> {
        let d = self.acceleration_device(true)?;
        dst.check(d, self.domain)?;
        src.check(d, self.domain)?;
        if mode > 1 || ptr::eq(dst, src) || dst.kind != 3 && dst.kind != src.kind {
            return Err(INVALID);
        }
        let info = vk::VkCopyAccelerationStructureInfoKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_COPY_ACCELERATION_STRUCTURE_INFO_KHR,
            src: src.handle,
            dst: dst.handle,
            mode,
            ..Default::default()
        };
        unsafe {
            (d.acceleration_commands.copy.ok_or(UNSUPPORTED)?)(self.command, &info);
        };
        Ok(())
    }
    pub(in crate::foundation) fn serialize_acceleration(
        &self,
        object: &Acceleration,
        range: DeviceRange,
        write: bool,
    ) -> Result<(), Status> {
        let d = self.acceleration_device(true)?;
        object.check(d, self.domain)?;
        if range.address == 0 || range.address % 256 != 0 || range.size == 0 {
            return Err(INVALID);
        }
        range.address.checked_add(range.size).ok_or(INVALID)?;
        unsafe {
            if write {
                let info=vk::VkCopyAccelerationStructureToMemoryInfoKHR {sType:vk::VkStructureType_VK_STRUCTURE_TYPE_COPY_ACCELERATION_STRUCTURE_TO_MEMORY_INFO_KHR,
                src:object.handle,dst:vk::VkDeviceOrHostAddressKHR {deviceAddress:range.address},mode:vk::VkCopyAccelerationStructureModeKHR_VK_COPY_ACCELERATION_STRUCTURE_MODE_SERIALIZE_KHR,..Default::default()};
                (d.acceleration_commands.serialize.ok_or(UNSUPPORTED)?)(self.command, &info);
            } else {
                let info=vk::VkCopyMemoryToAccelerationStructureInfoKHR {sType:vk::VkStructureType_VK_STRUCTURE_TYPE_COPY_MEMORY_TO_ACCELERATION_STRUCTURE_INFO_KHR,
                dst:object.handle,src:vk::VkDeviceOrHostAddressConstKHR {deviceAddress:range.address},mode:vk::VkCopyAccelerationStructureModeKHR_VK_COPY_ACCELERATION_STRUCTURE_MODE_DESERIALIZE_KHR,..Default::default()};
                (d.acceleration_commands.deserialize.ok_or(UNSUPPORTED)?)(self.command, &info);
            }
        }
        Ok(())
    }
    pub(in crate::foundation) unsafe fn acceleration_properties(
        &self,
        objects: &[*mut Acceleration],
        pool: &QueryPool,
        first: u32,
        scratch: HostSpan,
    ) -> Result<(), Status> {
        let d = self.acceleration_device(false)?;
        let count = u32::try_from(objects.len()).map_err(|_| INVALID)?;
        pool.range(d, first, count)?;
        let query_type = acceleration::query_type(d, pool.kind)?;
        let (layout, offsets) = acceleration::layout(count, 0)?;
        let base = acceleration::workspace(scratch, layout)?;
        let handles = unsafe {
            base.add(offsets[5])
                .cast::<vk::VkAccelerationStructureKHR>()
        };
        for (i, p) in objects.iter().enumerate() {
            let object = unsafe { p.as_ref() }.ok_or(INVALID)?;
            object.check(d, self.domain)?;
            unsafe {
                handles.add(i).write(object.handle);
            }
        }
        unsafe {
            (d.acceleration_commands.properties.ok_or(UNSUPPORTED)?)(
                self.command,
                count,
                handles,
                query_type,
                pool.handle,
                first,
            );
        }
        Ok(())
    }
}
