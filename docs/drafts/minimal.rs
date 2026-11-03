//! C1 cross-family record integration. Design only, not runtime code.
use crate::arguments::ArgumentLayout;
use crate::descriptors::DescriptorHeap;
use crate::presentation::Record;

pub const INHERITANCE_DESC: u32 = 3002;
pub const GENERATED_ARGUMENT: u32 = 2;
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Inheritance {
    pub header: Record,
    pub occlusion: u32,
    pub query_flags: u32,
    pub statistics: u64,
    pub rendering: *const Record,
    pub resource_heap: *mut DescriptorHeap,
    pub sampler_heap: *mut DescriptorHeap,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GeneratedToken {
    pub kind: u32,
    pub offset: u32,
    pub argument_offset: u32,
    pub argument_size: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GeneratedLayoutDesc {
    pub header: Record,
    pub arguments: *mut ArgumentLayout,
    pub stages: u64,
    pub flags: u32,
    pub stride: u32,
    pub token_count: u32,
    pub reserved: u32,
    pub tokens: *const GeneratedToken,
}
