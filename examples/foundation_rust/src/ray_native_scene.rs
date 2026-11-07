//! Direct Vulkan triangle/instance build for the native combined-usage control.
use super::*;
#[repr(C)]
struct Instance {
    transform: [f32; 12],
    custom_mask: u32,
    table_flags: u32,
    reference: u64,
}
pub struct Scene {
    bottom: vk::VkAccelerationStructureKHR,
    top: vk::VkAccelerationStructureKHR,
    pub address: u64,
}
impl Scene {
    pub unsafe fn record(
        loader: &Loader,
        d: vk::VkDevice,
        cmd: vk::VkCommandBuffer,
        base: u64,
        host: *mut u8,
        alignment: u64,
    ) -> gpu::Result<Self> {
        unsafe {
            macro_rules! load {
                ($n:ident, $t:ident) => {{
                    let f: vk::$t = std::mem::transmute(loader.proc(
                        std::ffi::CStr::from_bytes_with_nul_unchecked(
                            concat!(stringify!($n), "\0").as_bytes(),
                        ),
                    ));
                    f.ok_or(UNSUPPORTED)?
                }};
            }
            let sizes = load!(
                vkGetAccelerationStructureBuildSizesKHR,
                PFN_vkGetAccelerationStructureBuildSizesKHR
            );
            let create = load!(
                vkCreateAccelerationStructure2KHR,
                PFN_vkCreateAccelerationStructure2KHR
            );
            let address = load!(
                vkGetAccelerationStructureDeviceAddressKHR,
                PFN_vkGetAccelerationStructureDeviceAddressKHR
            );
            let build = load!(
                vkCmdBuildAccelerationStructuresKHR,
                PFN_vkCmdBuildAccelerationStructuresKHR
            );
            let barrier = load!(vkCmdPipelineBarrier2, PFN_vkCmdPipelineBarrier2);
            let vertices = [
                0f32, 0., 0., 1., 0., 0., 0., 1., 0., 2., 0., 0., 3., 0., 0., 2., 1., 0.,
            ];
            ptr::copy_nonoverlapping(vertices.as_ptr(), host.cast::<f32>(), vertices.len());
            let geometry = [0,36].map(|offset| vk::VkAccelerationStructureGeometryKHR {
                sType: vk::VkStructureType_VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_GEOMETRY_KHR,
                geometryType: vk::VkGeometryTypeKHR_VK_GEOMETRY_TYPE_TRIANGLES_KHR,
                flags: vk::VkGeometryFlagBitsKHR_VK_GEOMETRY_OPAQUE_BIT_KHR,
                geometry: vk::VkAccelerationStructureGeometryDataKHR { triangles: vk::VkAccelerationStructureGeometryTrianglesDataKHR {
                    sType: vk::VkStructureType_VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_GEOMETRY_TRIANGLES_DATA_KHR,
                    vertexFormat: vk::VkFormat_VK_FORMAT_R32G32B32_SFLOAT,
                    vertexData: vk::VkDeviceOrHostAddressConstKHR { deviceAddress: base + offset },
                    vertexStride: 12, maxVertex: 2, indexType: vk::VkIndexType_VK_INDEX_TYPE_NONE_KHR, ..Default::default()
                } }, ..Default::default()
            });
            let instances = vk::VkAccelerationStructureGeometryKHR { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_GEOMETRY_KHR,
                geometryType: vk::VkGeometryTypeKHR_VK_GEOMETRY_TYPE_INSTANCES_KHR,
                geometry: vk::VkAccelerationStructureGeometryDataKHR { instances: vk::VkAccelerationStructureGeometryInstancesDataKHR {
                    sType: vk::VkStructureType_VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_GEOMETRY_INSTANCES_DATA_KHR,
                    data: vk::VkDeviceOrHostAddressConstKHR { deviceAddress: base + 256 }, ..Default::default()
                } }, ..Default::default()
            };
            let mut bottom = vk::VkAccelerationStructureBuildGeometryInfoKHR { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_BUILD_GEOMETRY_INFO_KHR,
                type_: vk::VkAccelerationStructureTypeKHR_VK_ACCELERATION_STRUCTURE_TYPE_BOTTOM_LEVEL_KHR,
                flags: vk::VkBuildAccelerationStructureFlagBitsKHR_VK_BUILD_ACCELERATION_STRUCTURE_ALLOW_UPDATE_BIT_KHR | vk::VkBuildAccelerationStructureFlagBitsKHR_VK_BUILD_ACCELERATION_STRUCTURE_ALLOW_COMPACTION_BIT_KHR,
                mode: vk::VkBuildAccelerationStructureModeKHR_VK_BUILD_ACCELERATION_STRUCTURE_MODE_BUILD_KHR,
                geometryCount: 2, pGeometries: geometry.as_ptr(), scratchData: vk::VkDeviceOrHostAddressKHR { deviceAddress: base + 262144 }, ..Default::default()
            };
            let mut top = vk::VkAccelerationStructureBuildGeometryInfoKHR {
                type_:
                    vk::VkAccelerationStructureTypeKHR_VK_ACCELERATION_STRUCTURE_TYPE_TOP_LEVEL_KHR,
                flags: 0,
                geometryCount: 1,
                pGeometries: &instances,
                ..bottom
            };
            assert_eq!((base + 262144) % alignment, 0);
            let max = [1, 1];
            let make = |info: &vk::VkAccelerationStructureBuildGeometryInfoKHR,
                        offset|
             -> gpu::Result<vk::VkAccelerationStructureKHR> {
                let mut size = vk::VkAccelerationStructureBuildSizesInfoKHR { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_BUILD_SIZES_INFO_KHR, ..Default::default() };
                sizes(d, vk::VkAccelerationStructureBuildTypeKHR_VK_ACCELERATION_STRUCTURE_BUILD_TYPE_DEVICE_KHR, info, max.as_ptr(), &mut size);
                assert!(size.accelerationStructureSize <= 65536 && size.buildScratchSize <= 786432);
                let mut result = ptr::null_mut();
                check(create(d, &vk::VkAccelerationStructureCreateInfo2KHR { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_CREATE_INFO_2_KHR,
                    addressRange: vk::VkDeviceAddressRangeKHR { address: base + offset, size: size.accelerationStructureSize },
                    addressFlags: vk::VkAddressCommandFlagBitsKHR_VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR | vk::VkAddressCommandFlagBitsKHR_VK_ADDRESS_COMMAND_STORAGE_BUFFER_USAGE_BIT_KHR,
                    type_: info.type_, ..Default::default() }, ptr::null(), &mut result))?;
                Ok(result)
            };
            let b = make(&bottom, 65536)?;
            let t = make(&top, 131072)?;
            let query = |handle| {
                address(d, &vk::VkAccelerationStructureDeviceAddressInfoKHR { sType: vk::VkStructureType_VK_STRUCTURE_TYPE_ACCELERATION_STRUCTURE_DEVICE_ADDRESS_INFO_KHR, accelerationStructure: handle, ..Default::default() })
            };
            host.add(256).cast::<Instance>().write(Instance {
                transform: [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0.],
                custom_mask: 37 | (255 << 24),
                table_flags: 1 << 24,
                reference: query(b),
            });
            bottom.dstAccelerationStructure = b;
            top.dstAccelerationStructure = t;
            let ranges = [vk::VkAccelerationStructureBuildRangeInfoKHR {
                primitiveCount: 1,
                ..Default::default()
            }; 2];
            let range = ranges.as_ptr();
            let edge = |after, dst| {
                let memory = vk::VkMemoryBarrier2 {
                    sType: vk::VkStructureType_VK_STRUCTURE_TYPE_MEMORY_BARRIER_2,
                    srcStageMask: vk::VK_PIPELINE_STAGE_2_ACCELERATION_STRUCTURE_BUILD_BIT_KHR,
                    dstStageMask: after,
                    srcAccessMask: vk::VK_ACCESS_2_ACCELERATION_STRUCTURE_WRITE_BIT_KHR,
                    dstAccessMask: dst,
                    ..Default::default()
                };
                barrier(
                    cmd,
                    &vk::VkDependencyInfo {
                        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_DEPENDENCY_INFO,
                        memoryBarrierCount: 1,
                        pMemoryBarriers: &memory,
                        ..Default::default()
                    },
                );
            };
            build(cmd, 1, &bottom, &range);
            edge(
                vk::VK_PIPELINE_STAGE_2_ACCELERATION_STRUCTURE_BUILD_BIT_KHR,
                vk::VK_ACCESS_2_ACCELERATION_STRUCTURE_READ_BIT_KHR
                    | vk::VK_ACCESS_2_ACCELERATION_STRUCTURE_WRITE_BIT_KHR,
            );
            build(cmd, 1, &top, &range);
            edge(
                vk::VK_PIPELINE_STAGE_2_RAY_TRACING_SHADER_BIT_KHR,
                vk::VK_ACCESS_2_ACCELERATION_STRUCTURE_READ_BIT_KHR,
            );
            Ok(Self {
                bottom: b,
                top: t,
                address: query(t),
            })
        }
    }
    pub unsafe fn destroy(self, loader: &Loader, d: vk::VkDevice) {
        let destroy: vk::PFN_vkDestroyAccelerationStructureKHR =
            unsafe { std::mem::transmute(loader.proc(c"vkDestroyAccelerationStructureKHR")) };
        unsafe {
            destroy.unwrap()(d, self.top, ptr::null());
            destroy.unwrap()(d, self.bottom, ptr::null());
        }
    }
}
