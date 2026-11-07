//! Direct native trace commands. Table allocation/layout and stack policy are caller work.
use super::*;
fn table(
    input: RayTable,
    limits: &RayLimits,
    raygen: bool,
) -> Result<vk::VkStridedDeviceAddressRegionKHR, Status> {
    input
        .range
        .address
        .checked_add(input.range.size)
        .ok_or(INVALID)?;
    if input.range.address % u64::from(limits.table_alignment) != 0 {
        return Err(INVALID);
    }
    if raygen {
        if input.range.address == 0
            || input.range.size != input.stride
            || input.range.size < u64::from(limits.group_handle_size)
        {
            return Err(INVALID);
        }
    } else {
        // A zero stride repeats the same shader record and is a native facility.
        if input.stride % u64::from(limits.group_handle_alignment) != 0 {
            return Err(INVALID);
        }
        if input.stride > u64::from(limits.max_table_stride) {
            return Err(UNSUPPORTED);
        }
        if input.range.address != 0 && input.range.size < u64::from(limits.group_handle_size) {
            return Err(INVALID);
        }
    }
    Ok(vk::VkStridedDeviceAddressRegionKHR {
        deviceAddress: input.range.address,
        stride: input.stride,
        size: input.range.size,
    })
}
fn dimensions(size: Extent, execution: &ExecutionLimits, ray: &RayLimits) -> Result<(), Status> {
    let axes = [size.x, size.y, size.z];
    for (i, &n) in axes.iter().enumerate() {
        if u64::from(n)
            > u64::from(execution.max_groups[i]) * u64::from(execution.max_local_size[i])
        {
            return Err(UNSUPPORTED);
        }
    }
    if axes
        .into_iter()
        .try_fold(1u64, |v, n| v.checked_mul(u64::from(n)))
        .ok_or(UNSUPPORTED)?
        > u64::from(ray.max_trace_invocations)
    {
        return Err(UNSUPPORTED);
    }
    Ok(())
}
fn indirect(range: DeviceRange, size: u64) -> Result<(), Status> {
    if range.address == 0 || range.address % 4 != 0 || range.size < size {
        return Err(INVALID);
    }
    range.address.checked_add(range.size).ok_or(INVALID)?;
    Ok(())
}
impl List {
    pub(in crate::foundation) fn set_ray_stack_size(&self, bytes: u32) -> Result<(), Status> {
        let d = self.outside_render()?;
        stages(d, self.domain, STAGE_RAY, false)?;
        // Native dynamic state can be set before binding a pipeline.
        unsafe {
            (d.ray_commands.set_stack.ok_or(UNSUPPORTED)?)(self.command, bytes);
        }
        Ok(())
    }
    pub(in crate::foundation) fn trace_rays(
        &self,
        tables: &RayTables,
        size: Extent,
        args: Option<DeviceRange>,
    ) -> Result<(), Status> {
        let d = self.outside_render()?;
        self.executable(STAGE_RAY)?;
        let limits = &d.snapshot.ray_limits;
        let tables = [
            table(tables.raygen, limits, true)?,
            table(tables.miss, limits, false)?,
            table(tables.hit, limits, false)?,
            table(tables.callable, limits, false)?,
        ];
        if let Some(args) = args {
            if d.snapshot.features.enabled & RAY_INDIRECT == 0 {
                return Err(UNSUPPORTED);
            }
            indirect(args, 12)?;
            unsafe {
                (d.ray_commands.indirect.ok_or(UNSUPPORTED)?)(
                    self.command,
                    &tables[0],
                    &tables[1],
                    &tables[2],
                    &tables[3],
                    args.address,
                );
            }
        } else {
            dimensions(size, &d.snapshot.execution_limits, limits)?;
            unsafe {
                (d.ray_commands.trace.ok_or(UNSUPPORTED)?)(
                    self.command,
                    &tables[0],
                    &tables[1],
                    &tables[2],
                    &tables[3],
                    size.x,
                    size.y,
                    size.z,
                );
            }
        }
        Ok(())
    }
    pub(in crate::foundation) fn trace_rays_indirect_table(
        &self,
        args: DeviceRange,
    ) -> Result<(), Status> {
        let d = self.outside_render()?;
        self.executable(STAGE_RAY)?;
        if d.snapshot.features.enabled & RAY_INDIRECT_TABLE == 0 {
            return Err(UNSUPPORTED);
        }
        indirect(args, size_of::<RayIndirect>() as u64)?;
        unsafe {
            (d.ray_commands.indirect_table.ok_or(UNSUPPORTED)?)(self.command, args.address);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shader_tables_preserve_zero_stride_and_raygen_record_size() {
        let limits = RayLimits {
            group_handle_size: 32,
            group_handle_alignment: 16,
            table_alignment: 64,
            max_table_stride: 4096,
            ..Default::default()
        };
        let t = RayTable {
            range: DeviceRange {
                address: 256,
                size: 32,
            },
            stride: 0,
        };
        assert_eq!(table(t, &limits, false).unwrap().stride, 0);
        assert!(matches!(table(t, &limits, true), Err(INVALID)));
        assert!(table(RayTable { stride: 32, ..t }, &limits, true).is_ok());
        assert!(matches!(
            table(RayTable { stride: 17, ..t }, &limits, false),
            Err(INVALID)
        ));
        assert!(table(
            RayTable {
                range: DeviceRange {
                    address: 256,
                    size: 8192
                },
                stride: 8192
            },
            &limits,
            true
        )
        .is_ok());
        assert!(matches!(
            table(RayTable { stride: 8192, ..t }, &limits, false),
            Err(UNSUPPORTED)
        ));
        assert_eq!(
            size_of::<RayIndirect>(),
            size_of::<vk::VkTraceRaysIndirectCommand2KHR>()
        );
        assert_eq!(
            std::mem::offset_of!(RayIndirect, width),
            std::mem::offset_of!(vk::VkTraceRaysIndirectCommand2KHR, width)
        );
        assert_eq!(
            indirect(
                DeviceRange {
                    address: 256,
                    size: 11
                },
                12
            ),
            Err(INVALID)
        );
        assert_eq!(
            indirect(
                DeviceRange {
                    address: 256,
                    size: 12
                },
                12
            ),
            Ok(())
        );
    }
}
