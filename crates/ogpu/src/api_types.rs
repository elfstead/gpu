//! Single definitions of backend-independent C data structures.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct OgpuBufferDesc {
    pub size_bytes: u64,
    pub placement: u32,
    pub extra_usage: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct OgpuRasterDesc {
    pub push_size_bytes: u32,
    pub topology: u32,
    pub color_format: u32,
    pub depth_format: u32,
    pub depth_test: u32,
    pub depth_write: u32,
    pub depth_compare: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct OgpuColorAttachment {
    pub image: *mut crate::OgpuImage,
    pub load: u32,
    pub store: u32,
    pub clear: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct OgpuDepthAttachment {
    pub image: *mut crate::OgpuImage,
    pub load: u32,
    pub store: u32,
    pub clear: f32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct OgpuRenderingDesc {
    pub color: OgpuColorAttachment,
    pub depth: OgpuDepthAttachment,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct OgpuIndexRange {
    pub buffer: *mut crate::OgpuBuffer,
    pub offset: u64,
    pub size_bytes: u64,
    pub format: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct OgpuDrawIndexedArguments {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub vertex_offset: i32,
    pub first_instance: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OgpuDeviceLimits {
    pub max_group_size: [u32; 3],
    pub max_group_invocations: u32,
    pub max_shared_memory_bytes: u32,
    pub max_dispatch: [u32; 3],
    pub max_image_1d: u32,
    pub max_image_2d: u32,
    pub max_push_data_bytes: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct OgpuTimingInfo {
    pub timestamp_period_ns: f64,
    pub timestamp_valid_bits: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct OgpuImageDesc {
    pub dimension: u32,
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub usage: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct OgpuSamplerDesc {
    pub min_filter: u32,
    pub mag_filter: u32,
    pub address_u: u32,
    pub address_v: u32,
}

#[repr(C)]
pub struct OgpuImageEntry {
    pub image: *const crate::OgpuImage,
    pub kind: u32,
    pub reserved: u32,
}

#[repr(C)]
pub struct OgpuDrawArguments {
    pub vertex_count: u32,
    pub instance_count: u32,
    pub first_vertex: u32,
    pub first_instance: u32,
}
/// Borrowed mapped storage; no ownership or synchronization is conferred.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct OgpuHostView {
    pub data: *mut std::ffi::c_void,
    pub size_bytes: u64,
    pub alignment: u64,
    pub access_granularity: u64,
    pub coherent: u64,
}
