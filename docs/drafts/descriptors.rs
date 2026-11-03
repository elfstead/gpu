//! C1 descriptor record mirror. Design only; not runtime or SDK code.
//! See ../minimal-core-descriptors.md and ogpu_descriptors.h.
use std::ffi::c_void;

pub const HEAP_HOST: u32 = 0;
pub const HEAP_MEMORY: u32 = 1;
pub const HEAP_NATIVE: u32 = 2;
pub const HEAP_SHADER_VISIBLE: u32 = 1;
pub const HEAP_CPU_WRITE: u32 = 2;
pub const HEAP_CPU_COPY_SOURCE: u32 = 4;
pub const HEAP_CPU_COPY_DESTINATION: u32 = 8;
pub const DESCRIPTOR_WRITE_RESOURCE: u32 = 0;
pub const DESCRIPTOR_WRITE_SAMPLER: u32 = 1;
pub const DESCRIPTOR_COPY: u32 = 2;

// Record-check stand-ins for existing opaque Memory and new DescriptorHeap.
// Runtime integration uses the actual types, without a foreign-function call.
#[repr(C)]
pub struct Memory { _private: [u8; 0] }
#[repr(C)]
pub struct DescriptorHeap { _private: [u8; 0] }

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Span {
    pub memory: *mut Memory,
    pub offset: u64,
    pub size: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct HostSpan {
    pub data: *mut c_void,
    pub size: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct HeapRequest {
    pub kind: u32,
    pub storage: u32,
    pub usage: u32,
    pub flags: u32,
    pub capacity: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct HeapRequirements {
    pub unit_bytes: u64,
    pub backing_size: u64,
    pub backing_alignment: u64,
    pub reserved_size: u64,
    pub reserved_alignment: u64,
    pub memory_usage: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct HeapDesc {
    pub request: HeapRequest,
    pub memory: Span,
    pub host: HostSpan,
    pub reserved_offset: u64,
    pub reserved_size: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DescriptorShape {
    pub size: u64,
    pub alignment: u64,
    pub array_stride: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct HeapLocation {
    pub heap: *mut DescriptorHeap,
    pub offset: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DescriptorCopy {
    pub source: HeapLocation,
    pub destination: HeapLocation,
    pub size: u64,
}
