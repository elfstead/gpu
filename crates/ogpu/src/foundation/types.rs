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
