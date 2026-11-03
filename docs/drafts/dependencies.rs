//! C1 dependency/submission records. Design only, not runtime or SDK code.
use crate::descriptors::Span;
use crate::presentation::{Binary, Image};

pub const QUERY_DEPENDENCY_LIMITS: u32 = 19;
pub const DEPENDENCY_SPLIT: u32 = 1;
pub const DEPENDENCY_OWNERSHIP: u32 = 2;
pub const DEPENDENCY_BINARY: u32 = 4;
pub const MEMORY_SCOPE_NONE: u32 = 0;
pub const MEMORY_SCOPE_OBJECT: u32 = 1;
pub const MEMORY_SCOPE_BYTES: u32 = 2;
pub const SUBMIT_REJECTED: u32 = 0;
pub const SUBMIT_ACCEPTED: u32 = 1;
pub const SUBMIT_UNKNOWN: u32 = 2;

#[repr(C)] pub struct Split { _private: [u8; 0] }
#[repr(C)] pub struct List { _private: [u8; 0] }
#[repr(C)] pub struct Timeline { _private: [u8; 0] }
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Point { pub timeline: *mut Timeline, pub value: u64 }
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SyncPoint { pub point: Point, pub stages: u64 }
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BinaryPoint { pub binary: *mut Binary, pub stages: u64 }
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Subresources {
    pub aspects: u32,
    pub first_mip: u32,
    pub mip_count: u32,
    pub first_layer: u32,
    pub layer_count: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DependencyLimits {
    pub domain: u32,
    pub flags: u32,
    pub memory_scope: u32,
    pub reserved: u32,
    pub wait_stages: u64,
    pub signal_stages: u64,
    pub split_stages: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BarrierScope {
    pub stages_before: u64,
    pub stages_after: u64,
    pub access_before: u64,
    pub access_after: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MemoryBarrier {
    pub scope: BarrierScope,
    pub range: Span,
    pub source_domain: u32,
    pub destination_domain: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ImageBarrier {
    pub scope: BarrierScope,
    pub image: *mut Image,
    pub range: Subresources,
    pub old_state: u32,
    pub new_state: u32,
    pub source_domain: u32,
    pub destination_domain: u32,
    pub discard: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Dependency {
    pub global_count: u32,
    pub memory_count: u32,
    pub image_count: u32,
    pub flags: u32,
    pub global: *const BarrierScope,
    pub memory: *const MemoryBarrier,
    pub images: *const ImageBarrier,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SplitDependency {
    pub split: *const Split,
    pub dependency: *const Dependency,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SubmitDesc {
    pub list_count: u32,
    pub wait_count: u32,
    pub signal_count: u32,
    pub binary_wait_count: u32,
    pub binary_signal_count: u32,
    pub flags: u32,
    pub lists: *const *mut List,
    pub waits: *const SyncPoint,
    pub signals: *const SyncPoint,
    pub binary_waits: *const BinaryPoint,
    pub binary_signals: *const BinaryPoint,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SubmitOutcome { pub status: i32, pub acceptance: u32 }
