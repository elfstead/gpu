//! Caller-backed acceleration interpretations and cold native build requirements.
use super::*;
use std::alloc::Layout;
pub struct Acceleration {
    pub(super) device: *const Device,
    pub(super) handle: vk::VkAccelerationStructureKHR,
    pub(super) kind: u32,
    pub(super) backing: Span,
}
pub(super) struct Commands {
    pub create: vk::PFN_vkCreateAccelerationStructure2KHR,
    pub destroy: vk::PFN_vkDestroyAccelerationStructureKHR,
    pub address: vk::PFN_vkGetAccelerationStructureDeviceAddressKHR,
    pub sizes: vk::PFN_vkGetAccelerationStructureBuildSizesKHR,
    pub build: vk::PFN_vkCmdBuildAccelerationStructuresKHR,
    pub indirect: vk::PFN_vkCmdBuildAccelerationStructuresIndirectKHR,
    pub copy: vk::PFN_vkCmdCopyAccelerationStructureKHR,
    pub serialize: vk::PFN_vkCmdCopyAccelerationStructureToMemoryKHR,
    pub deserialize: vk::PFN_vkCmdCopyMemoryToAccelerationStructureKHR,
    pub properties: vk::PFN_vkCmdWriteAccelerationStructuresPropertiesKHR,
    pub compatibility: vk::PFN_vkGetDeviceAccelerationStructureCompatibilityKHR,
}
impl Commands {
    pub fn load(instance: &Instance, enabled: FeatureBits) -> Result<Self, Status> {
        macro_rules! get {
            ($name:literal,$ty:ty,$feature:expr) => {{
                if enabled & $feature == 0 {
                    None
                } else {
                    let f = unsafe {
                        std::mem::transmute::<vk::PFN_vkVoidFunction, $ty>(instance.proc($name))
                    };
                    if f.is_none() {
                        return Err(UNSUPPORTED);
                    }
                    f
                }
            }};
        }
        Ok(Self {
            create: get!(
                c"vkCreateAccelerationStructure2KHR",
                vk::PFN_vkCreateAccelerationStructure2KHR,
                ACCELERATION_STRUCTURE
            ),
            destroy: get!(
                c"vkDestroyAccelerationStructureKHR",
                vk::PFN_vkDestroyAccelerationStructureKHR,
                ACCELERATION_STRUCTURE
            ),
            address: get!(
                c"vkGetAccelerationStructureDeviceAddressKHR",
                vk::PFN_vkGetAccelerationStructureDeviceAddressKHR,
                ACCELERATION_STRUCTURE
            ),
            sizes: get!(
                c"vkGetAccelerationStructureBuildSizesKHR",
                vk::PFN_vkGetAccelerationStructureBuildSizesKHR,
                ACCELERATION_STRUCTURE
            ),
            build: get!(
                c"vkCmdBuildAccelerationStructuresKHR",
                vk::PFN_vkCmdBuildAccelerationStructuresKHR,
                ACCELERATION_STRUCTURE
            ),
            indirect: get!(
                c"vkCmdBuildAccelerationStructuresIndirectKHR",
                vk::PFN_vkCmdBuildAccelerationStructuresIndirectKHR,
                ACCELERATION_INDIRECT_BUILD
            ),
            copy: get!(
                c"vkCmdCopyAccelerationStructureKHR",
                vk::PFN_vkCmdCopyAccelerationStructureKHR,
                ACCELERATION_STRUCTURE
            ),
            serialize: get!(
                c"vkCmdCopyAccelerationStructureToMemoryKHR",
                vk::PFN_vkCmdCopyAccelerationStructureToMemoryKHR,
                ACCELERATION_STRUCTURE
            ),
            deserialize: get!(
                c"vkCmdCopyMemoryToAccelerationStructureKHR",
                vk::PFN_vkCmdCopyMemoryToAccelerationStructureKHR,
                ACCELERATION_STRUCTURE
            ),
            properties: get!(
                c"vkCmdWriteAccelerationStructuresPropertiesKHR",
                vk::PFN_vkCmdWriteAccelerationStructuresPropertiesKHR,
                ACCELERATION_STRUCTURE
            ),
            compatibility: get!(
                c"vkGetDeviceAccelerationStructureCompatibilityKHR",
                vk::PFN_vkGetDeviceAccelerationStructureCompatibilityKHR,
                ACCELERATION_STRUCTURE
            ),
        })
    }
}
pub(in crate::foundation) fn enabled(d: &Device) -> Result<(), Status> {
    d.ready()?;
    if d.snapshot.features.enabled & ACCELERATION_STRUCTURE == 0 {
        Err(UNSUPPORTED)
    } else {
        Ok(())
    }
}
pub(super) fn kind(value: u32, object: bool) -> Result<vk::VkAccelerationStructureTypeKHR, Status> {
    match value {
        1 => Ok(vk::VkAccelerationStructureTypeKHR_VK_ACCELERATION_STRUCTURE_TYPE_TOP_LEVEL_KHR),
        2 => Ok(vk::VkAccelerationStructureTypeKHR_VK_ACCELERATION_STRUCTURE_TYPE_BOTTOM_LEVEL_KHR),
        3 if object => {
            Ok(vk::VkAccelerationStructureTypeKHR_VK_ACCELERATION_STRUCTURE_TYPE_GENERIC_KHR)
        }
        _ => Err(INVALID),
    }
}
pub(super) fn build_flags(flags: u32) -> Result<u32, Status> {
    if flags & !31 != 0 {
        return Err(UNSUPPORTED);
    }
    if flags & 12 == 12 {
        return Err(INVALID);
    }
    Ok(flags)
}
pub(super) fn layout(builds: u32, geometries: u32) -> Result<(Layout, [usize; 6]), Status> {
    let mut layout =
        Layout::array::<vk::VkAccelerationStructureBuildGeometryInfoKHR>(builds as usize)
            .map_err(|_| INVALID)?;
    let parts = [
        Layout::array::<vk::VkAccelerationStructureGeometryKHR>(geometries as usize),
        Layout::array::<*const vk::VkAccelerationStructureBuildRangeInfoKHR>(builds as usize),
        Layout::array::<u64>(builds as usize),
        Layout::array::<u32>(builds as usize),
        Layout::array::<*const u32>(builds as usize),
        Layout::array::<vk::VkAccelerationStructureKHR>(builds as usize),
    ];
    let mut offsets = [0; 6];
    for (i, part) in parts.into_iter().enumerate() {
        let (next, offset) = layout
            .extend(part.map_err(|_| INVALID)?)
            .map_err(|_| INVALID)?;
        layout = next;
        offsets[i] = offset;
    }
    Ok((layout.pad_to_align(), offsets))
}
pub(in crate::foundation) fn scratch_requirements(
    builds: u32,
    geometries: u32,
) -> Result<HostRequirements, Status> {
    let (layout, _) = layout(builds, geometries)?;
    Ok(HostRequirements {
        size: layout.size() as u64,
        alignment: layout.align() as u64,
    })
}
pub(super) fn workspace(scratch: HostSpan, layout: Layout) -> Result<*mut u8, Status> {
    if scratch.size < layout.size() as u64 {
        return Err(CAPACITY);
    }
    if layout.size() != 0 && (scratch.data.is_null() || scratch.data as usize % layout.align() != 0)
    {
        return Err(INVALID);
    }
    Ok(scratch.data.cast())
}
// Shared direct native representation; no allocation or address registry.
pub(super) unsafe fn prepare(
    d: &Device,
    desc: &AccelerationBuildDesc,
    output: *mut vk::VkAccelerationStructureGeometryKHR,
) -> Result<vk::VkAccelerationStructureBuildGeometryInfoKHR, Status> {
    enabled(d)?;
    desc.header
        .validate::<AccelerationBuildDesc>(ACCELERATION_BUILD_DESC)?;
    let native_kind = kind(desc.kind, false)?;
    let flags = build_flags(desc.flags)?;
    if desc.reserved != 0 || (desc.kind == 1 && desc.geometry_count != 1) {
        return Err(INVALID);
    }
    if u64::from(desc.geometry_count) > d.snapshot.ray_limits.max_geometries {
        return Err(UNSUPPORTED);
    }
    let geometry = unsafe { command_slice(desc.geometries, desc.geometry_count)? };
    let counts = unsafe { command_slice(desc.max_primitives, desc.geometry_count)? };
    let mut total = 0u64;
    for (i, (g, &max)) in geometry.iter().zip(counts).enumerate() {
        if g.flags & !3 != 0 {
            return Err(UNSUPPORTED);
        }
        if (desc.kind == 1 && g.kind != 3)
            || (desc.kind == 2 && (g.kind == 3 || i != 0 && g.kind != geometry[0].kind))
        {
            return Err(INVALID);
        }
        total = total.checked_add(u64::from(max)).ok_or(INVALID)?;
        let data = match g.kind {
            1 => {
                let t = unsafe { g.data.triangles };
                if t.reserved != 0 || t.stride > u64::from(u32::MAX) || t.transform % 16 != 0 {
                    return Err(INVALID);
                }
                let format = images::format(t.format)?;
                if !d
                    .snapshot
                    .formats
                    .iter()
                    .any(|f| f.format == t.format && f.buffer_features & 16 != 0)
                {
                    return Err(UNSUPPORTED);
                }
                let index = match t.index_type {
                    0 => vk::VkIndexType_VK_INDEX_TYPE_NONE_KHR,
                    16 => vk::VkIndexType_VK_INDEX_TYPE_UINT16,
                    32 => vk::VkIndexType_VK_INDEX_TYPE_UINT32,
                    _ => return Err(INVALID),
                };
                if t.index_type == 0 && t.indices != 0
                    || t.index_type == 16 && t.indices % 2 != 0
                    || t.index_type == 32 && t.indices % 4 != 0
                {
                    return Err(INVALID);
                }
                vk::VkAccelerationStructureGeometryDataKHR {triangles:vk::VkAccelerationStructureGeometryTrianglesDataKHR {
                    sType:vk::VkStructureType_VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_GEOMETRY_TRIANGLES_DATA_KHR,
                    vertexFormat:format.native,vertexData:vk::VkDeviceOrHostAddressConstKHR {deviceAddress:t.vertices},vertexStride:t.stride,maxVertex:t.max_vertex,
                    indexType:index,indexData:vk::VkDeviceOrHostAddressConstKHR {deviceAddress:t.indices},transformData:vk::VkDeviceOrHostAddressConstKHR {deviceAddress:t.transform},..Default::default()}}
            }
            2 => {
                let a = unsafe { g.data.aabbs };
                if a.stride % 8 != 0 || a.data % 8 != 0 || a.stride > u64::from(u32::MAX) {
                    return Err(INVALID);
                }
                vk::VkAccelerationStructureGeometryDataKHR {aabbs:vk::VkAccelerationStructureGeometryAabbsDataKHR {
                    sType:vk::VkStructureType_VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_GEOMETRY_AABBS_DATA_KHR,data:vk::VkDeviceOrHostAddressConstKHR {deviceAddress:a.data},stride:a.stride,..Default::default()}}
            }
            3 => {
                let a = unsafe { g.data.instances };
                if a.pointers > 1
                    || a.reserved != 0
                    || a.data % (if a.pointers == 1 { 8 } else { 16 }) != 0
                {
                    return Err(INVALID);
                }
                vk::VkAccelerationStructureGeometryDataKHR {instances:vk::VkAccelerationStructureGeometryInstancesDataKHR {
                    sType:vk::VkStructureType_VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_GEOMETRY_INSTANCES_DATA_KHR,arrayOfPointers:a.pointers,data:vk::VkDeviceOrHostAddressConstKHR {deviceAddress:a.data},..Default::default()}}
            }
            _ => return Err(UNSUPPORTED),
        };
        unsafe {
            output.add(i).write(vk::VkAccelerationStructureGeometryKHR {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_GEOMETRY_KHR,
                geometryType: g.kind - 1,
                geometry: data,
                flags: g.flags,
                ..Default::default()
            });
        }
    }
    if total
        > if desc.kind == 1 {
            d.snapshot.ray_limits.max_instances
        } else {
            d.snapshot.ray_limits.max_primitives
        }
    {
        return Err(UNSUPPORTED);
    }
    Ok(vk::VkAccelerationStructureBuildGeometryInfoKHR {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_BUILD_GEOMETRY_INFO_KHR,
        type_: native_kind,
        flags,
        geometryCount: desc.geometry_count,
        pGeometries: output,
        ..Default::default()
    })
}
pub(in crate::foundation) unsafe fn requirements(
    d: &Device,
    desc: &AccelerationBuildDesc,
    scratch: HostSpan,
) -> Result<AccelerationRequirements, Status> {
    let (layout, offsets) = layout(1, desc.geometry_count)?;
    let base = workspace(scratch, layout)?;
    let info = unsafe { prepare(d, desc, base.add(offsets[0]).cast())? };
    let mut sizes = vk::VkAccelerationStructureBuildSizesInfoKHR {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_BUILD_SIZES_INFO_KHR,
        ..Default::default()
    };
    unsafe {
        (d.acceleration_commands.sizes.ok_or(UNSUPPORTED)?)(
            d.handle,
            vk::VkAccelerationStructureBuildTypeKHR_VK_ACCELERATION_STRUCTURE_BUILD_TYPE_DEVICE_KHR,
            &info,
            desc.max_primitives,
            &mut sizes,
        );
    }
    Ok(AccelerationRequirements {
        storage_size: sizes.accelerationStructureSize,
        storage_alignment: d.snapshot.ray_limits.storage_alignment,
        build_scratch_size: sizes.buildScratchSize,
        update_scratch_size: sizes.updateScratchSize,
        scratch_alignment: d.snapshot.ray_limits.scratch_alignment,
    })
}
impl Acceleration {
    pub(in crate::foundation) unsafe fn create(
        d: &Device,
        desc: &AccelerationDesc,
    ) -> Result<Box<Self>, Status> {
        enabled(d)?;
        desc.header
            .validate::<AccelerationDesc>(ACCELERATION_DESC)?;
        if desc.reserved != 0 {
            return Err(INVALID);
        }
        let m = unsafe { desc.backing.memory.as_ref() }.ok_or(INVALID)?;
        let (_, _, address) = m.descriptor_range(
            d,
            desc.backing.offset,
            desc.backing.size,
            ACCELERATION_STORAGE_USAGE,
        )?;
        if address % 256 != 0 || desc.backing.size == 0 {
            return Err(INVALID);
        }
        let info = vk::VkAccelerationStructureCreateInfo2KHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_CREATE_INFO_2_KHR,
            addressRange: vk::VkDeviceAddressRangeKHR {
                address,
                size: desc.backing.size,
            },
            addressFlags: m.capture_address_flags(),
            type_: kind(desc.kind, true)?,
            ..Default::default()
        };
        let mut handle = ptr::null_mut();
        unsafe {
            d.result((d.acceleration_commands.create.ok_or(UNSUPPORTED)?)(
                d.handle,
                &info,
                ptr::null(),
                &mut handle,
            ))?;
        }
        Ok(Box::new(Self {
            device: d,
            handle,
            kind: desc.kind,
            backing: desc.backing,
        }))
    }
    pub(in crate::foundation) fn address(&self) -> Result<u64, Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        let info = vk::VkAccelerationStructureDeviceAddressInfoKHR {
            sType:
                vk::VkStructureType_VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_DEVICE_ADDRESS_INFO_KHR,
            accelerationStructure: self.handle,
            ..Default::default()
        };
        Ok(unsafe { (d.acceleration_commands.address.ok_or(UNSUPPORTED)?)(d.handle, &info) })
    }
    pub(super) fn check(&self, d: &Device, domain: u32) -> Result<(), Status> {
        if !ptr::eq(self.device, d) {
            return Err(INVALID);
        }
        unsafe { &*self.backing.memory }.command_range(
            d,
            domain,
            self.backing.offset,
            self.backing.size,
            ACCELERATION_STORAGE_USAGE,
        )?;
        Ok(())
    }
}
impl Drop for Acceleration {
    fn drop(&mut self) {
        let d = unsafe { &*self.device };
        unsafe {
            (d.acceleration_commands.destroy.unwrap())(d.handle, self.handle, ptr::null());
        }
    }
}
pub(super) fn query_type(d: &Device, kind: u32) -> Result<vk::VkQueryType, Status> {
    enabled(d)?;
    if kind >= 7 && d.snapshot.features.enabled & RAY_MAINTENANCE == 0 {
        return Err(UNSUPPORTED);
    }
    match kind {5=>Ok(vk::VkQueryType_VK_QUERY_TYPE_ACCELERATION_STRUCTURE_COMPACTED_SIZE_KHR),
        6=>Ok(vk::VkQueryType_VK_QUERY_TYPE_ACCELERATION_STRUCTURE_SERIALIZATION_SIZE_KHR),
        7=>Ok(vk::VkQueryType_VK_QUERY_TYPE_ACCELERATION_STRUCTURE_SIZE_KHR),
        8=>Ok(vk::VkQueryType_VK_QUERY_TYPE_ACCELERATION_STRUCTURE_SERIALIZATION_BOTTOM_LEVEL_POINTERS_KHR),_=>Err(INVALID)}
}
impl Device {
    pub(in crate::foundation) unsafe fn acceleration_compatibility(
        &self,
        version: *const u8,
    ) -> Result<bool, Status> {
        let info = vk::VkAccelerationStructureVersionInfoKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_VERSION_INFO_KHR,
            pVersionData: version,
            ..Default::default()
        };
        let mut result=vk::VkAccelerationStructureCompatibilityKHR_VK_ACCELERATION_STRUCTURE_COMPATIBILITY_INCOMPATIBLE_KHR;
        unsafe {
            (self
                .acceleration_commands
                .compatibility
                .ok_or(UNSUPPORTED)?)(self.handle, &info, &mut result);
        }
        Ok(result==vk::VkAccelerationStructureCompatibilityKHR_VK_ACCELERATION_STRUCTURE_COMPATIBILITY_COMPATIBLE_KHR)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn range_and_instance_are_native_data_abis() {
        assert_eq!(
            size_of::<AccelerationRange>(),
            size_of::<vk::VkAccelerationStructureBuildRangeInfoKHR>()
        );
        assert_eq!(
            align_of::<AccelerationRange>(),
            align_of::<vk::VkAccelerationStructureBuildRangeInfoKHR>()
        );
        let r = AccelerationRange {
            primitive_count: 3,
            primitive_offset: 20,
            first_vertex: 7,
            transform_offset: 32,
        };
        let native =
            unsafe { &*ptr::from_ref(&r).cast::<vk::VkAccelerationStructureBuildRangeInfoKHR>() };
        assert_eq!(
            (
                native.primitiveCount,
                native.primitiveOffset,
                native.firstVertex,
                native.transformOffset
            ),
            (3, 20, 7, 32)
        );
        assert_eq!(size_of::<AccelerationInstance>(), 64);
        assert_eq!(
            std::mem::offset_of!(AccelerationInstance, custom_index_and_mask),
            48
        );
        assert_eq!(
            std::mem::offset_of!(AccelerationInstance, table_offset_and_flags),
            52
        );
        assert_eq!(std::mem::offset_of!(AccelerationInstance, reference), 56);
    }
    #[test]
    fn translation_storage_and_native_build_choices_are_explicit() {
        for (builds, geometry) in [(0, 0), (1, 1), (3, 7), (17, 0)] {
            let (l, o) = layout(builds, geometry).unwrap();
            let sizes = [
                size_of::<vk::VkAccelerationStructureGeometryKHR>() * geometry as usize,
                size_of::<*const vk::VkAccelerationStructureBuildRangeInfoKHR>() * builds as usize,
                8 * builds as usize,
                4 * builds as usize,
                size_of::<*const u32>() * builds as usize,
                size_of::<vk::VkAccelerationStructureKHR>() * builds as usize,
            ];
            assert!(
                o[0] >= size_of::<vk::VkAccelerationStructureBuildGeometryInfoKHR>()
                    * builds as usize
            );
            for i in 0..5 {
                assert!(o[i] + sizes[i] <= o[i + 1]);
            }
            assert!(o[5] + sizes[5] <= l.size());
            if l.size() != 0 {
                assert_eq!(
                    workspace(
                        HostSpan {
                            data: ptr::null_mut(),
                            size: l.size() as u64 - 1
                        },
                        l
                    ),
                    Err(CAPACITY)
                );
                assert_eq!(
                    workspace(
                        HostSpan {
                            data: ptr::null_mut(),
                            size: l.size() as u64
                        },
                        l
                    ),
                    Err(INVALID)
                );
            }
        }
        for f in [0, 1, 2, 3, 4, 8, 16, 19] {
            assert_eq!(build_flags(f), Ok(f));
        }
        assert_eq!(build_flags(12), Err(INVALID));
        assert_eq!(build_flags(32), Err(UNSUPPORTED));
        assert!(kind(3, true).is_ok());
        assert_eq!(kind(3, false), Err(INVALID));
    }
}
