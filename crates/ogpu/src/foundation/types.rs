//! C records for include/ogpu_next.h. No backend types or implicit owners.
use super::{Adapter, Device, Discovery, Queue, Timeline};
use std::ffi::{c_char, c_void};

pub const VERSION: u32 = 1;
pub type Status = i32;
pub const OK: Status = 0;
pub const TIMEOUT: Status = 2;
pub const INVALID: Status = -1;
pub const UNSUPPORTED: Status = -2;
pub const OUT_OF_MEMORY: Status = -3;
pub const DEVICE_LOST: Status = -4;
pub const CAPACITY: Status = -5;
pub const BACKEND_ERROR: Status = -6;
pub const VERSION_MISMATCH: Status = -7;
pub const INTERNAL_ERROR: Status = -8;
pub const INFO: u32 = 1;
pub const QUEUES: u32 = 2;
pub const MEMORY_TYPES: u32 = 3;
pub const MEMORY_HEAPS: u32 = 4;
pub const FEATURES: u32 = 5;
pub const DEVICE_DESC: u32 = 100;
pub const MEMORY_DESC: u32 = 101;
pub const ARENA_DESC: u32 = 102;
pub const RECORDING_DESC: u32 = 103;
pub const SUBMIT_DESC: u32 = 104;
pub const DEPENDENCY: u32 = 105;
pub const IMAGE_DESC: u32 = 106;
pub const VIEW_DESC: u32 = 107;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Extent {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ImageDesc {
    pub header: Record,
    pub format: u32,
    pub dimension: u32,
    pub mip_count: u32,
    pub layer_count: u32,
    pub sample_count: u32,
    pub extent: Extent,
    pub usage: u64,
    pub flags: u32,
    pub view_formats: *const u32,
    pub view_format_count: u32,
    pub concurrent_domain_count: u32,
    pub concurrent_domains: *const u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Subresources {
    pub aspects: u32,
    pub first_mip: u32,
    pub mip_count: u32,
    pub first_layer: u32,
    pub layer_count: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ViewDesc {
    pub header: Record,
    pub format: u32,
    pub dimension: u32,
    pub usage: u32,
    pub component_mapping: [u32; 4],
    pub range: Subresources,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Offset {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ImageRegion {
    pub aspect: u32,
    pub mip: u32,
    pub first_layer: u32,
    pub layer_count: u32,
    pub offset: Offset,
    pub extent: Extent,
}
#[repr(C)]
pub struct ImageCopy {
    pub region: ImageRegion,
    pub row_pitch: u64,
    pub slice_pitch: u64,
    pub state: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DepthStencil {
    pub depth: f32,
    pub stencil: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub union ClearValue {
    pub f32: [f32; 4],
    pub u32: [u32; 4],
    pub i32: [i32; 4],
    pub depth_stencil: DepthStencil,
}
#[repr(C)]
pub struct ImageBarrier {
    pub image: *mut super::Image,
    pub range: Subresources,
    pub before: u64,
    pub after: u64,
    pub old_state: u32,
    pub new_state: u32,
    pub source_domain: u32,
    pub destination_domain: u32,
    pub discard: u32,
}
pub const MEMORY_LIMITS: u32 = 6;
pub const RASTER: u64 = 1;
pub const FLOAT16: u64 = 2;
pub const UNIFIED_IMAGES: u64 = 4;
pub const GRAPHICS: u32 = 1;
pub const COMPUTE: u32 = 2;
pub const TRANSFER: u32 = 4;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Record {
    pub kind: u32,
    pub version: u32,
    pub byte_size: u32,
    pub flags: u32,
    pub next: *const Record,
}
impl Record {
    pub fn new<T>(kind: u32) -> Self {
        Self {
            kind,
            version: VERSION,
            byte_size: size_of::<T>() as u32,
            flags: 0,
            next: std::ptr::null(),
        }
    }
    pub(super) fn validate<T>(&self, kind: u32) -> Result<(), Status> {
        if self.kind != kind || self.version != VERSION {
            return Err(UNSUPPORTED);
        }
        if self.byte_size != size_of::<T>() as u32 || self.flags != 0 || !self.next.is_null() {
            return Err(INVALID);
        }
        Ok(())
    }
}
#[repr(C)]
pub struct Query {
    pub header: Record,
    pub data: *mut c_void,
    pub capacity: u32,
    pub count: u32,
    pub element_size: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct AdapterInfo {
    pub name: [c_char; 256],
    pub backend: u32,
    pub vendor_id: u32,
    pub device_id: u32,
    pub device_type: u32,
    pub native_api_version: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct QueueInfo {
    pub domain: u32,
    pub flags: u32,
    pub count: u32,
    pub timestamp_bits: u32,
    pub copy_granularity: [u32; 3],
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MemoryTypeInfo {
    pub id: u32,
    pub heap: u32,
    pub properties: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MemoryHeapInfo {
    pub size: u64,
    pub id: u32,
    pub device_local: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct FeatureInfo {
    pub available: u64,
    pub enabled: u64,
    pub max_timeline_difference: u64,
    pub baseline_supported: u32,
    pub device_scope: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct QueueRequest {
    pub domain: u32,
    pub count: u32,
    pub priority: f32,
}
#[repr(C)]
pub struct DeviceDesc {
    pub header: Record,
    pub queues: *const QueueRequest,
    pub queue_request_count: u32,
    pub reserved: u32,
    pub required_features: u64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Point {
    pub timeline: *mut Timeline,
    pub value: u64,
}

#[repr(C)]
pub struct ArenaDesc {
    pub header: Record,
    pub domain: u32,
    pub list_capacity: u32,
}
#[repr(C)]
pub struct RecordingDesc {
    pub header: Record,
    pub replay_mode: u32,
    pub level: u32,
    pub inheritance: *const Record,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostRequirements {
    pub size: u64,
    pub alignment: u64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SyncPoint {
    pub point: Point,
    pub stages: u64,
}
#[repr(C)]
pub struct SubmitDesc {
    pub header: Record,
    pub list_count: u32,
    pub wait_count: u32,
    pub signal_count: u32,
    pub lists: *const *mut super::List,
    pub waits: *const SyncPoint,
    pub signals: *const SyncPoint,
    pub scratch: *mut c_void,
    pub scratch_size: u64,
}
#[repr(C)]
pub struct MemoryBarrier {
    pub range: Span,
    pub before: u64,
    pub after: u64,
    pub source_domain: u32,
    pub destination_domain: u32,
}
#[repr(C)]
pub struct Dependency {
    pub header: Record,
    pub before: u64,
    pub after: u64,
    pub global_before: u64,
    pub global_after: u64,
    pub memory_count: u32,
    pub image_count: u32,
    pub flags: u32,
    pub memory: *const MemoryBarrier,
    pub images: *const ImageBarrier,
    pub scratch: *mut c_void,
    pub scratch_size: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct MemoryLimits {
    pub max_buffer_size: u64,
    pub max_allocation_size: u64,
    pub cache_atom_size: u64,
    pub map_alignment: u64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MemoryDesc {
    pub header: Record,
    pub size: u64,
    pub alignment: u64,
    pub usage: u64,
    pub memory_type: u32,
    pub kind: u32,
    pub concurrent_domains: *const u32,
    pub concurrent_domain_count: u32,
    pub flags: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Span {
    pub memory: *mut super::Memory,
    pub offset: u64,
    pub size: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Requirements {
    pub size: u64,
    pub alignment: u64,
    pub dedicated_required: u32,
    pub dedicated_preferred: u32,
    pub compatible_type_count: u32,
    pub compatible_type_capacity: u32,
    pub compatible_types: *mut u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Mapping {
    pub data: *mut std::ffi::c_void,
    pub size: u64,
    pub cache_atom_size: u64,
    pub cache_offset: u64,
    pub coherent: u32,
    pub reserved: u32,
}

// Keep C opaque types reachable without exposing their internals.
pub type DiscoveryHandle = *mut Discovery;
pub type AdapterHandle = *const Adapter;
pub type DeviceHandle = *mut Device;
pub type QueueHandle = *mut Queue;

pub(super) fn validate_requests(
    families: &[QueueInfo],
    requests: &[QueueRequest],
) -> Result<(), Status> {
    if requests.is_empty() {
        return Err(INVALID);
    }
    for (index, request) in requests.iter().enumerate() {
        if request.count == 0
            || !request.priority.is_finite()
            || !(0.0..=1.0).contains(&request.priority)
            || requests[..index].iter().any(|r| r.domain == request.domain)
        {
            return Err(INVALID);
        }
        let family = families
            .iter()
            .find(|q| q.domain == request.domain)
            .ok_or(UNSUPPORTED)?;
        if request.count > family.count {
            return Err(UNSUPPORTED);
        }
    }
    Ok(())
}
