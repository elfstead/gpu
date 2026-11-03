//! C1 presentation record mirror. Design only; no runtime or C forwarding.
use std::ffi::c_void;
use crate::descriptors::HostSpan;

pub const PRESENT_VULKAN: u32 = 1;
pub const PRESENT_DXGI: u32 = 2;
pub const PRESENTATION_WIN32: u32 = 2;
pub const OCCLUDED: i32 = 5;
pub const STILL_DRAWING: i32 = 6;
pub const WIN32_SURFACE_DESC: u32 = 3000;
pub const DXGI_CHAIN_DESC: u32 = 3001;
pub const AVAILABILITY_SIGNALS: u32 = 1;
pub const AVAILABILITY_QUEUE_ORDERED: u32 = 2;
pub const PRESENT_REPLACE: u32 = 1;
pub const PRESENT_RESIZE: u32 = 2;
pub const PRESENT_RETIRE_FENCE: u32 = 4;
pub const PRESENT_PACING_HANDLE: u32 = 8;
pub const PRESENT_BINARY_WAIT: u32 = 16;
pub const CHAIN_NONE: u32 = 0;
pub const CHAIN_ACTIVE: u32 = 1;
pub const CHAIN_RETIRED: u32 = 2;
pub const CHAIN_NO_IMAGES: u32 = 3;
pub const CHAIN_LOST: u32 = 4;
pub const IMAGES_UNCHANGED: u32 = 0;
pub const IMAGES_REPLACED: u32 = 1;
pub const IMAGES_INVALIDATED: u32 = 2;
pub const FRAME_RETAINED: u32 = 0;
pub const FRAME_RELEASED: u32 = 1;
pub const FRAME_UNKNOWN: u32 = 2;
pub const EFFECT_NONE: u32 = 0;
pub const EFFECT_ENQUEUED: u32 = 1;
pub const EFFECT_UNKNOWN: u32 = 2;
pub const WAITABLE_WIN32: u32 = 1;

// Record-check stand-ins; integration uses actual shared Rust object types.
#[repr(C)] pub struct Surface { _private: [u8; 0] }
#[repr(C)] pub struct Queue { _private: [u8; 0] }
#[repr(C)] pub struct Swapchain { _private: [u8; 0] }
#[repr(C)] pub struct Image { _private: [u8; 0] }
#[repr(C)] pub struct Binary { _private: [u8; 0] }
#[repr(C)] pub struct Fence { _private: [u8; 0] }

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Record {
    pub kind: u32,
    pub version: u32,
    pub byte_size: u32,
    pub flags: u32,
    pub next: *const Record,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Win32SurfaceDesc {
    pub header: Record,
    pub hwnd: *mut c_void,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DxgiChainDesc {
    pub header: Record,
    pub surface: *mut Surface,
    pub queue: *mut Queue,
    pub format: u32,
    pub extent: [u32; 2],
    pub image_count: u32,
    pub usage: u64,
    pub swap_effect: u32,
    pub scaling: u32,
    pub alpha: u32,
    pub color_space: u32,
    pub chain_flags: u32,
    pub maximum_frame_latency: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PresentationContract {
    pub profile: u32,
    pub availability: u32,
    pub capabilities: u32,
    pub max_present_count: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ChainInfo {
    pub generation: u64,
    pub state: u32,
    pub image_count: u32,
    pub ordered_queue: *mut Queue,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ChainCreateOutcome {
    pub chain: *mut Swapchain,
    pub previous_state: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ChainResizeDesc {
    pub format: u32,
    pub extent: [u32; 2],
    pub image_count: u32,
    pub chain_flags: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ChainResizeOutcome {
    pub generation: u64,
    pub state: u32,
    pub image_effect: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct AcquireDesc {
    pub timeout_ns: u64,
    pub binary: *mut Binary,
    pub fence: *mut Fence,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub chain: *mut Swapchain,
    pub image: *mut Image,
    pub generation: u64,
    pub index: u32,
    pub availability: u32,
    pub ordered_queue: *mut Queue,
    pub binary: *mut Binary,
    pub fence: *mut Fence,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PresentItem {
    pub frame: Frame,
    pub retire_fence: *mut Fence,
    pub sync_interval: u32,
    pub native_flags: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PresentResult {
    pub status: i32,
    pub frame_effect: u32,
    pub fence_effect: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PresentDesc {
    pub wait_count: u32,
    pub item_count: u32,
    pub waits: *const *mut Binary,
    pub items: *const PresentItem,
    pub results: *mut PresentResult,
    pub scratch: HostSpan,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PresentOutcome {
    pub work_effect: u32,
    pub wait_effect: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PacingHandle {
    pub kind: u32,
    pub reserved: u32,
    pub handle: *mut c_void,
}
