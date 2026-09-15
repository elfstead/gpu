//! Single definitions of backend-independent C data structures.
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
