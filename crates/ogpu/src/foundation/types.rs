//! C records for include/ogpu_next.h. No backend types or implicit owners.
use super::features::*;
use super::{
    Adapter, Binary, Device, Discovery, Fence, Image, Queue, Surface, Swapchain, Timeline,
};
use std::ffi::{c_char, c_void};

/// Backend-independent format IDs, matching the C header. An ID does not imply
/// device support: query format features and the exact image profile before use.
pub mod image_formats {
    macro_rules! declare_formats {
        ($($name:ident = $id:literal => ($native:ident, $aspects:literal, $class:literal, $numeric:ident, $depth:literal, ($width:literal, $height:literal, $bytes:literal)),)*) => {
            $(pub const $name: u32 = $id;)*
            #[cfg(test)]
            #[test]
            fn c_ids_match_catalog() {
                let header = include_str!("../../../../include/ogpu_next.h");
                let mut expected = 1;
                $(assert_eq!($id, expected); expected += 1;
                  assert!(header.contains(&format!("OGPU_NEXT_{} = {},", stringify!($name), $id)));)*
                assert_eq!(expected, 173);
            }
        };
    }
    crate::foundation::format_catalog::format_catalog!(declare_formats);
}

pub const VERSION: u32 = 9;
/// Opaque optional-feature identifiers. They are not bit flags.
pub mod feature_ids {
    pub const RASTER: u32 = 1;
    pub const FLOAT16: u32 = 2;
    pub const UNIFIED_IMAGES: u32 = 3;
    pub const SAMPLER_ANISOTROPY: u32 = 4;
    pub const CACHE_CONTROL: u32 = 5;
    pub const INT8: u32 = 6;
    pub const INT16: u32 = 7;
    pub const INT64: u32 = 8;
    pub const FLOAT64: u32 = 9;
    pub const STORAGE8: u32 = 10;
    pub const BUFFER_ATOMIC64: u32 = 11;
    pub const SHARED_ATOMIC64: u32 = 12;
    pub const SUBGROUP_SIZE_CONTROL: u32 = 13;
    pub const FULL_SUBGROUPS: u32 = 14;
    pub const SUBGROUP_EXTENDED_TYPES: u32 = 15;
    pub const PIPELINE_STATISTICS: u32 = 16;
    pub const PRECISE_OCCLUSION: u32 = 17;
    pub const INHERITED_QUERIES: u32 = 18;
    pub const VERTEX_STORES_ATOMICS: u32 = 19;
    pub const FRAGMENT_STORES_ATOMICS: u32 = 20;
    pub const INDIRECT_FIRST_INSTANCE: u32 = 21;
    pub const INDEPENDENT_BLEND: u32 = 22;
    pub const DEPTH_CLAMP: u32 = 23;
    pub const DEPTH_BIAS_CLAMP: u32 = 24;
    pub const NON_SOLID_FILL: u32 = 25;
    pub const WIDE_LINES: u32 = 26;
    pub const DEPTH_BOUNDS: u32 = 27;
    pub const SAMPLE_SHADING: u32 = 28;
    pub const ALPHA_TO_ONE: u32 = 29;
    pub const UNUSED_ATTACHMENTS: u32 = 30;
    pub const LOGIC_OP: u32 = 31;
    pub const DUAL_SOURCE_BLEND: u32 = 32;
    pub const SHADER_OUTPUT_LAYER: u32 = 33;
    pub const BUFFER_FLOAT32_ATOMICS: u32 = 34;
    pub const BUFFER_FLOAT32_ATOMIC_ADD: u32 = 35;
    pub const BUFFER_FLOAT32_ATOMIC_MIN_MAX: u32 = 36;
    pub const SHARED_FLOAT32_ATOMICS: u32 = 37;
    pub const SHARED_FLOAT32_ATOMIC_ADD: u32 = 38;
    pub const SHARED_FLOAT32_ATOMIC_MIN_MAX: u32 = 39;
    pub const BUFFER_FLOAT64_ATOMICS: u32 = 40;
    pub const BUFFER_FLOAT64_ATOMIC_ADD: u32 = 41;
    pub const BUFFER_FLOAT64_ATOMIC_MIN_MAX: u32 = 42;
    pub const SHARED_FLOAT64_ATOMICS: u32 = 43;
    pub const SHARED_FLOAT64_ATOMIC_ADD: u32 = 44;
    pub const SHARED_FLOAT64_ATOMIC_MIN_MAX: u32 = 45;
    pub const COOPERATIVE_MATRIX: u32 = 46;
    pub const VULKAN_MEMORY_MODEL: u32 = 47;
    pub const VULKAN_MEMORY_MODEL_DEVICE_SCOPE: u32 = 48;
    pub const INTEGER_DOT: u32 = 49;
    pub const BUFFER_FLOAT16_ATOMICS: u32 = 50;
    pub const BUFFER_FLOAT16_ATOMIC_ADD: u32 = 51;
    pub const BUFFER_FLOAT16_ATOMIC_MIN_MAX: u32 = 52;
    pub const SHARED_FLOAT16_ATOMICS: u32 = 53;
    pub const SHARED_FLOAT16_ATOMIC_ADD: u32 = 54;
    pub const SHARED_FLOAT16_ATOMIC_MIN_MAX: u32 = 55;
    pub const VULKAN_MEMORY_MODEL_CHAINS: u32 = 56;
    pub const CUBE_ARRAY: u32 = 57;
    pub const PRESENTATION: u32 = 58;
    pub const MESH_SHADER: u32 = 59;
    pub const TASK_SHADER: u32 = 60;
    pub const GENERATED_COMMANDS: u32 = 61;
}
/// Borrowed feature IDs. Input storage lives through its host call. Query output
/// storage lives through the queried adapter/discovery or device lifetime.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Features {
    pub ids: *const u32,
    pub count: u32,
    pub reserved: u32,
}
impl Default for Features {
    fn default() -> Self {
        Self::from_slice(&[])
    }
}
impl Features {
    pub fn from_slice(ids: &[u32]) -> Self {
        Self {
            ids: if ids.is_empty() {
                std::ptr::null()
            } else {
                ids.as_ptr()
            },
            count: u32::try_from(ids.len()).expect("feature count exceeds u32"),
            reserved: 0,
        }
    }
    /// # Safety
    /// Nonempty storage must contain count readable IDs for the returned lifetime.
    pub unsafe fn as_slice<'a>(&self) -> Result<&'a [u32], Status> {
        if self.reserved != 0
            || (self.count != 0 && self.ids.is_null())
            || u64::from(self.count) > isize::MAX as u64 / 4
        {
            return Err(INVALID);
        }
        if self.count == 0 {
            Ok(&[])
        } else {
            Ok(unsafe { std::slice::from_raw_parts(self.ids, self.count as usize) })
        }
    }
}

pub type Status = i32;
pub const OK: Status = 0;
pub const NOT_READY: Status = 1;
pub const TIMEOUT: Status = 2;
pub const COMPILE_REQUIRED: Status = 3;
pub const SUBOPTIMAL: Status = 4;
pub const INVALID: Status = -1;
pub const UNSUPPORTED: Status = -2;
pub const OUT_OF_MEMORY: Status = -3;
pub const DEVICE_LOST: Status = -4;
pub const CAPACITY: Status = -5;
pub const BACKEND_ERROR: Status = -6;
pub const VERSION_MISMATCH: Status = -7;
pub const INTERNAL_ERROR: Status = -8;
pub const SURFACE_LOST: Status = -9;
pub const OUT_OF_DATE: Status = -10;
pub const PRESENTATION_WAYLAND: u32 = 1;
pub const MESH_STAGE: u64 = 1 << 20;
pub const TASK_STAGE: u64 = 1 << 21;
pub const MESH_LIMITS: u32 = 17;
pub const MESH_REQUIREMENTS: u32 = 129;
pub const GENERATED_LIMITS: u32 = 18;
pub const GENERATED_LAYOUT_DESC: u32 = 130;
pub const GENERATED_SET_DESC: u32 = 131;
pub const PREPROCESS_USAGE: u64 = 256;
pub const PREPROCESS_STAGE: u64 = 1 << 22;
pub const PREPROCESS_READ: u64 = 1 << 21;
pub const PREPROCESS_WRITE: u64 = 1 << 22;
pub const COMPILE_INDIRECT_BINDABLE: u32 = 2;
pub const GENERATED_EXECUTABLE: u32 = 1;
pub const GENERATED_ARGUMENT: u32 = 2;
pub const GENERATED_SEQUENCE_INDEX: u32 = 3;
pub const GENERATED_DISPATCH: u32 = 4;
pub const GENERATED_DRAW: u32 = 5;
pub const GENERATED_DRAW_INDEXED: u32 = 6;
pub const GENERATED_MESH: u32 = 7;
pub const GENERATED_DRAW_COUNT: u32 = 8;
pub const GENERATED_DRAW_INDEXED_COUNT: u32 = 9;
pub const GENERATED_MESH_COUNT: u32 = 10;
pub const GENERATED_VERTEX_BUFFER: u32 = 11;
pub const GENERATED_INDEX_BUFFER: u32 = 12;
pub const DYNAMIC_VERTEX_STRIDE: u64 = 4096;
pub const VERTEX_BIND_STRIDE: u32 = 1;
pub const GENERATED_EXPLICIT_PREPROCESS: u32 = 1;
pub const GENERATED_UNORDERED: u32 = 2;
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct GeneratedLimits {
    pub stages: u64,
    pub pipeline_binding_stages: u64,
    pub tokens: u64,
    pub max_pipelines: u32,
    pub max_sequences: u32,
    pub max_tokens: u32,
    pub max_token_offset: u32,
    pub max_stride: u32,
    pub reserved: u32,
    pub index_u16: u32,
    pub index_u32: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GeneratedToken {
    pub kind: u32,
    pub offset: u32,
    pub target: u32,
    pub size: u32,
}
#[repr(C)]
pub struct GeneratedLayoutDesc {
    pub header: Record,
    pub arguments: *mut super::ArgumentLayout,
    pub stages: u64,
    pub flags: u32,
    pub stride: u32,
    pub token_count: u32,
    pub reserved: u32,
    pub tokens: *const GeneratedToken,
}
#[repr(C)]
pub struct GeneratedSetDesc {
    pub header: Record,
    pub initial: *mut super::Executable,
    pub capacity: u32,
    pub reserved: u32,
}
#[repr(C)]
pub struct GeneratedSetWrite {
    pub index: u32,
    pub reserved: u32,
    pub executable: *mut super::Executable,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct GeneratedWork {
    pub layout: *mut super::GeneratedLayout,
    pub set: *mut super::GeneratedSet,
    pub executable: *mut super::Executable,
    pub max_sequences: u32,
    pub max_draws: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GeneratedRequirements {
    pub size: u64,
    pub alignment: u64,
    pub memory_type_bits: u32,
    pub reserved: u32,
}
#[repr(C)]
pub struct GeneratedBatch {
    pub work: GeneratedWork,
    pub arguments: Span,
    pub preprocess: Span,
    pub count: Span,
}
/// Nested-count GPU wire record; address is not an OGPU memory handle.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GeneratedDrawCount {
    pub address: u64,
    pub stride: u32,
    pub command_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct MeshWorkgroupLimits {
    pub max_groups: [u32; 3],
    pub max_total_groups: u32,
    pub max_local: [u32; 3],
    pub max_invocations: u32,
    pub max_shared_memory: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct MeshLimits {
    pub task: MeshWorkgroupLimits,
    pub mesh: MeshWorkgroupLimits,
    pub max_task_payload: u32,
    pub max_task_payload_shared: u32,
    pub max_mesh_payload_shared: u32,
    pub max_mesh_output_memory: u32,
    pub max_mesh_payload_output: u32,
    pub max_mesh_output_components: u32,
    pub max_mesh_output_vertices: u32,
    pub max_mesh_output_primitives: u32,
    pub max_mesh_output_layers: u32,
    pub max_mesh_multiview_views: u32,
    pub output_vertex_granularity: u32,
    pub output_primitive_granularity: u32,
    pub preferred_task_invocations: u32,
    pub preferred_mesh_invocations: u32,
    pub preference_flags: u32,
    pub reserved: u32,
}
#[repr(C)]
pub struct MeshRequirements {
    pub header: Record,
    pub features: Features,
    pub task_local: [u32; 3],
    pub task_shared: u32,
    pub mesh_local: [u32; 3],
    pub mesh_shared: u32,
    pub payload_size: u32,
    pub output_memory: u32,
    pub output_vertices: u32,
    pub output_primitives: u32,
    pub output_components: u32,
    pub output_layers: u32,
}
pub const WAYLAND_SURFACE_DESC: u32 = 125;
pub const SWAPCHAIN_DESC: u32 = 127;
pub const STATE_PRESENT: u32 = 8;
pub const SURFACE_CAPABILITIES: u32 = 1000;
pub const SURFACE_FORMATS: u32 = 1001;
pub const SURFACE_PRESENT_MODES: u32 = 1002;
pub const SURFACE_QUEUES: u32 = 1003;
pub const SURFACE_SCALING: u32 = 1004;
pub const SURFACE_COMPATIBLE_MODES: u32 = 1005;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WaylandSurfaceDesc {
    pub header: Record,
    pub display: *mut c_void,
    pub surface: *mut c_void,
}
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct SurfaceCapabilities {
    pub min_image_count: u32,
    pub max_image_count: u32,
    pub current_extent: [u32; 2],
    pub min_extent: [u32; 2],
    pub max_extent: [u32; 2],
    pub max_layers: u32,
    pub supported_transforms: u32,
    pub current_transform: u32,
    pub supported_alpha: u32,
    pub image_usage: u64,
    pub native_image_usage: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct SurfaceFormat {
    pub format: u32,
    pub native_format: u32,
    pub color_space: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct SurfaceScaling {
    pub scaling: u32,
    pub gravity_x: u32,
    pub gravity_y: u32,
    pub reserved: u32,
    pub min_extent: [u32; 2],
    pub max_extent: [u32; 2],
}
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct SurfaceQueue {
    pub domain: u32,
    pub supported: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct PresentationSupport {
    pub swapchain: u32,
    pub swapchain_maintenance: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SwapchainDesc {
    pub header: Record,
    pub surface: *mut super::Surface,
    pub old_swapchain: *mut super::Swapchain,
    pub format: u32,
    pub color_space: u32,
    pub extent: [u32; 2],
    pub layer_count: u32,
    pub min_image_count: u32,
    pub usage: u64,
    pub transform: u32,
    pub alpha: u32,
    pub present_mode: u32,
    pub clipped: u32,
    pub domain_count: u32,
    pub mode_count: u32,
    pub domains: *const u32,
    pub modes: *const u32,
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

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PresentationContract {
    pub profile: u32,
    pub availability: u32,
    pub capabilities: u32,
    pub max_present_count: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChainInfo {
    pub generation: u64,
    pub state: u32,
    pub image_count: u32,
    pub ordered_queue: *mut Queue,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChainCreateOutcome {
    pub chain: *mut Swapchain,
    pub previous_state: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChainResizeDesc {
    pub format: u32,
    pub extent: [u32; 2],
    pub image_count: u32,
    pub chain_flags: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChainResizeOutcome {
    pub generation: u64,
    pub state: u32,
    pub image_effect: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AcquireDesc {
    pub timeout_ns: u64,
    pub binary: *mut Binary,
    pub fence: *mut Fence,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PresentItem {
    pub frame: Frame,
    pub retire_fence: *mut Fence,
    pub sync_interval: u32,
    pub native_flags: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PresentResult {
    pub status: i32,
    pub frame_effect: u32,
    pub fence_effect: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PresentDesc {
    pub wait_count: u32,
    pub item_count: u32,
    pub waits: *const *mut Binary,
    pub items: *const PresentItem,
    pub results: *mut PresentResult,
    pub scratch: HostSpan,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PresentOutcome {
    pub work_effect: u32,
    pub wait_effect: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PacingHandle {
    pub kind: u32,
    pub reserved: u32,
    pub handle: *mut c_void,
}

/// Optional C function table. Native Rust callers use rust::presentation operations.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PresentationApi {
    pub discovery_create: Option<unsafe extern "C" fn(u32, *mut *mut Discovery) -> Status>,
    pub adapter_support:
        Option<unsafe extern "C" fn(*const Adapter, *mut PresentationSupport) -> Status>,
    pub surface_wayland_create: Option<
        unsafe extern "C" fn(
            *const Adapter,
            *const WaylandSurfaceDesc,
            *mut *mut super::Surface,
        ) -> Status,
    >,
    pub surface_destroy: Option<unsafe extern "C" fn(*mut super::Surface)>,
    pub surface_query:
        Option<unsafe extern "C" fn(*const Adapter, *const super::Surface, *mut Query) -> Status>,
    pub surface_mode_query: Option<
        unsafe extern "C" fn(*const Adapter, *const super::Surface, u32, *mut Query) -> Status,
    >,
    pub binary_create: Option<unsafe extern "C" fn(*mut Device, *mut *mut super::Binary) -> Status>,
    pub binary_destroy: Option<unsafe extern "C" fn(*mut super::Binary)>,
    pub fence_create:
        Option<unsafe extern "C" fn(*mut Device, u32, *mut *mut super::Fence) -> Status>,
    pub fence_destroy: Option<unsafe extern "C" fn(*mut super::Fence)>,
    pub fence_poll: Option<unsafe extern "C" fn(*const super::Fence) -> Status>,
    pub fence_wait: Option<unsafe extern "C" fn(*const super::Fence, u64) -> Status>,
    pub fence_reset: Option<unsafe extern "C" fn(*mut super::Fence) -> Status>,
    pub surface_win32_create: Option<
        unsafe extern "C" fn(*const Adapter, *const Win32SurfaceDesc, *mut *mut Surface) -> Status,
    >,
    pub presentation_contract_query: Option<
        unsafe extern "C" fn(
            *const Adapter,
            *const Surface,
            u32,
            *mut PresentationContract,
        ) -> Status,
    >,
    pub chain_create:
        Option<unsafe extern "C" fn(*mut Device, *const Record, *mut ChainCreateOutcome) -> Status>,
    pub chain_destroy: Option<unsafe extern "C" fn(*mut Swapchain)>,
    pub chain_info_query: Option<unsafe extern "C" fn(*const Swapchain, *mut ChainInfo)>,
    pub chain_images:
        Option<unsafe extern "C" fn(*const Swapchain, u32, *mut *mut Image, *mut u32) -> Status>,
    pub chain_resize: Option<
        unsafe extern "C" fn(
            *mut Swapchain,
            *const ChainResizeDesc,
            *mut ChainResizeOutcome,
        ) -> Status,
    >,
    pub chain_pacing_handle:
        Option<unsafe extern "C" fn(*const Swapchain, *mut PacingHandle) -> Status>,
    pub acquire:
        Option<unsafe extern "C" fn(*mut Swapchain, *const AcquireDesc, *mut Frame) -> Status>,
    pub abandon_scratch_requirements:
        Option<unsafe extern "C" fn(*const Device, u32, *mut HostRequirements) -> Status>,
    pub abandon: Option<
        unsafe extern "C" fn(*mut Swapchain, u32, *const Frame, HostSpan, *mut u32) -> Status,
    >,
    pub present_scratch_requirements:
        Option<unsafe extern "C" fn(*const Device, u32, u32, *mut HostRequirements) -> Status>,
    pub queue_present:
        Option<unsafe extern "C" fn(*mut Queue, *const PresentDesc, *mut PresentOutcome) -> Status>,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BinaryPoint {
    pub binary: *mut super::Binary,
    pub stages: u64,
}
// Names shared with ogpu_next.h for native Rust resource/transfer callers.
pub const MEMORY_HOST_VISIBLE: u32 = 2;
pub const MEMORY_COHERENT: u32 = 4;
pub const MEMORY_LINEAR: u32 = 1;
pub const MEMORY_OPAQUE: u32 = 2;
pub const USAGE_COPY_SRC: u64 = 1;
pub const USAGE_COPY_DST: u64 = 2;
pub const USAGE_INDIRECT: u64 = 4;
pub const USAGE_STORAGE: u64 = 64;
pub const STAGE_ALL: u64 = 1;
pub const STAGE_TRANSFER: u64 = 2;
pub const STAGE_COMPUTE: u64 = 4;
pub const STAGE_INDIRECT: u64 = 32;
pub const STAGE_HOST: u64 = 256;
pub const ACCESS_COPY_READ: u64 = 4;
pub const ACCESS_COPY_WRITE: u64 = 8;
pub const ACCESS_SHADER_READ: u64 = 16;
pub const ACCESS_SHADER_WRITE: u64 = 32;
pub const ACCESS_HOST_READ: u64 = 64;
pub const ACCESS_HOST_WRITE: u64 = 128;
pub const ACCESS_INDIRECT_READ: u64 = 256;
pub const RGBA8_UNORM: u32 = 3;
pub const IMAGE_2D: u32 = 2;
pub const VIEW_2D: u32 = 2;
pub const IMAGE_COPY_SRC: u64 = 1;
pub const IMAGE_COPY_DST: u64 = 2;
pub const IMAGE_SAMPLED: u64 = 4;
pub const IMAGE_COLOR_ATTACHMENT: u64 = 16;
pub const ASPECT_COLOR: u32 = 1;
pub const STATE_UNDEFINED: u32 = 0;
pub const STATE_GENERAL: u32 = 1;
pub const STATE_COPY_SRC: u32 = 2;
pub const STATE_COPY_DST: u32 = 3;
pub const DOMAIN_IGNORED: u32 = u32::MAX;
pub const EXECUTABLE_COMPUTE: u32 = 1;
pub const SHADER_SPIRV: u32 = 0;
pub const INFO: u32 = 1;
pub const QUEUES: u32 = 2;
pub const MEMORY_TYPES: u32 = 3;
pub const MEMORY_HEAPS: u32 = 4;
pub const FEATURES: u32 = 5;
pub const DEVICE_DESC: u32 = 100;
pub const MEMORY_DESC: u32 = 101;
pub const ARENA_DESC: u32 = 102;
pub const RECORDING_DESC: u32 = 103;
pub const IMAGE_DESC: u32 = 106;
pub const VIEW_DESC: u32 = 107;
pub const SAMPLER_DESC: u32 = 108;
pub const DESCRIPTOR_LIMITS: u32 = 7;
pub const EXECUTION_LIMITS: u32 = 8;
pub const EXECUTABLE_DESC: u32 = 110;
pub const SHADER_REQUIREMENTS: u32 = 112;
pub const SPECIALIZATION: u32 = 113;
pub const GRAPHICS_STATE: u32 = 114;
pub const RENDER_DESC: u32 = 115;
pub const VIEWPORT_STATE: u32 = 116;
pub const GRAPHICS_LIMITS: u32 = 9;
pub const QUERY_LIMITS: u32 = 10;
pub const QUERY_POOL_DESC: u32 = 117;
pub const EXECUTABLE_CACHE_DESC: u32 = 118;
pub const VERTEX_INPUT: u32 = 119;
pub(super) fn validate_feature_dependencies(required: u64) -> Result<(), Status> {
    if required & MESH_SHADER != 0 && required & RASTER == 0
        || required & TASK_SHADER != 0 && required & MESH_SHADER == 0
    {
        return Err(INVALID);
    }
    if required & (VULKAN_MEMORY_MODEL_DEVICE_SCOPE | VULKAN_MEMORY_MODEL_CHAINS) != 0
        && required & VULKAN_MEMORY_MODEL == 0
    {
        Err(INVALID)
    } else {
        Ok(())
    }
}
pub const MATRIX_TUPLES: u32 = 14;
pub const INTEGER_DOT_INFO: u32 = 15;
pub const MEMORY_BUDGETS: u32 = 16;
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct MemoryBudget {
    pub budget: u64,
    pub usage: u64,
    pub heap: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct IntegerDotInfo {
    pub accelerated: u32,
    pub accumulating_saturating_accelerated: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct MatrixTuple {
    pub m: u32,
    pub n: u32,
    pub k: u32,
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub result: u32,
    pub saturating: u32,
    pub scope: u32,
    pub reserved: u32,
    pub stages: u64,
}
pub const INHERITANCE: u32 = 121;
pub const RENDER_INHERITANCE: u32 = 122;
pub const STATISTICS_MASK: u64 = 2047;
pub const SUBGROUP_LIMITS: u32 = 11;
pub const SUBGROUP_STATE: u32 = 120;
pub const FORMATS: u32 = 12;
pub const FLOAT_CONTROLS: u32 = 13;
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct FloatControls {
    pub binary16: u32,
    pub binary32: u32,
    pub binary64: u32,
    pub denorm_independence: u32,
    pub rounding_independence: u32,
    pub reserved: u32,
}
pub const IMAGE_PROFILE: u32 = 123;
pub const RASTER_OPTIONS: u32 = 124;
/// Image-tiling capability bits returned by format discovery. These are physical
/// per-format facts, not device enables or promises about image profile tuples.
pub mod format_features {
    pub const SAMPLED: u64 = 1;
    pub const STORAGE: u64 = 2;
    pub const STORAGE_ATOMIC: u64 = 4;
    pub const COLOR: u64 = 8;
    pub const BLEND: u64 = 16;
    pub const DEPTH_STENCIL: u64 = 32;
    pub const BLIT_SRC: u64 = 64;
    pub const BLIT_DST: u64 = 128;
    pub const FILTER_LINEAR: u64 = 256;
    pub const COPY_SRC: u64 = 512;
    pub const COPY_DST: u64 = 1024;
    pub const STORAGE_READ_WITHOUT_FORMAT: u64 = 2048;
    pub const STORAGE_WRITE_WITHOUT_FORMAT: u64 = 4096;
}
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct FormatInfo {
    pub format: u32,
    pub aspects: u32,
    pub optimal_features: u64,
    pub linear_features: u64,
    pub buffer_features: u64,
    pub copy_block_extent: [u32; 3],
    pub copy_block_bytes: [u32; 3],
}
#[repr(C)]
pub struct ImageProfile {
    pub header: Record,
    pub format: u32,
    pub dimension: u32,
    pub usage: u64,
    pub image_flags: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct ImageLimits {
    pub max_extent: [u32; 3],
    pub max_mips: u32,
    pub max_layers: u32,
    pub sample_counts: u32,
    pub max_resource_size: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SubgroupLimits {
    pub default_size: u32,
    pub min_size: u32,
    pub max_size: u32,
    pub max_compute_workgroup_subgroups: u32,
    pub stages: u64,
    pub required_size_stages: u64,
    pub operations: u32,
    pub quad_all_stages: u32,
}
#[repr(C)]
pub struct SubgroupState {
    pub header: Record,
    pub operations: u32,
    pub required_size: u32,
    pub flags: u32,
    pub reserved: u32,
}
#[repr(C)]
pub struct ExecutableCacheDesc {
    pub header: Record,
    pub synchronization: u32,
    pub reserved: u32,
    pub initial: Bytes,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct QueryLimits {
    pub timestamp_period_ns: f32,
    pub timestamp_compute_graphics: u32,
    pub statistics: u64,
}
#[repr(C)]
pub struct QueryPoolDesc {
    pub header: Record,
    pub kind: u32,
    pub count: u32,
    pub statistics: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GraphicsLimits {
    pub max_colors: u32,
    pub max_width: u32,
    pub max_height: u32,
    pub max_layers: u32,
    pub max_viewport: [u32; 2],
    pub viewport_bounds: [f32; 2],
    pub color_samples: u32,
    pub depth_samples: u32,
    pub no_attachment_samples: u32,
    pub max_indirect_count: u32,
    pub integer_color_samples: u32,
    pub stencil_samples: u32,
    pub max_vertex_bindings: u32,
    pub max_vertex_attributes: u32,
    pub max_vertex_attribute_offset: u32,
    pub max_vertex_stride: u32,
    pub dynamic_states: u64,
    pub line_width_range: [f32; 2],
    pub line_width_granularity: f32,
    pub max_dual_source_attachments: u32,
    pub depth_resolve_modes: u32,
    pub stencil_resolve_modes: u32,
    pub independent_resolve_none: u32,
    pub independent_resolve: u32,
}
#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub struct ColorState {
    pub format: u32,
    pub write_mask: u32,
    pub blend: u32,
    pub src_color: u32,
    pub dst_color: u32,
    pub color_op: u32,
    pub src_alpha: u32,
    pub dst_alpha: u32,
    pub alpha_op: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct StencilState {
    pub fail: u32,
    pub pass: u32,
    pub depth_fail: u32,
    pub compare: u32,
    pub compare_mask: u32,
    pub write_mask: u32,
    pub reference: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct GraphicsState {
    pub header: Record,
    pub topology: u32,
    pub cull: u32,
    pub front_face: u32,
    pub samples: u32,
    pub color_count: u32,
    pub colors: *const ColorState,
    pub depth_format: u32,
    pub depth_test: u32,
    pub depth_write: u32,
    pub depth_compare: u32,
    pub blend_constants: [f32; 4],
    pub stencil_test: u32,
    pub stencil_front: StencilState,
    pub stencil_back: StencilState,
    pub vertex_input: *const Record,
    pub raster_options: *const Record,
    pub logic_op_enable: u32,
    pub logic_op: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RasterOptions {
    pub header: Record,
    pub primitive_restart: u32,
    pub depth_clamp: u32,
    pub discard: u32,
    pub polygon_mode: u32,
    pub depth_bias: u32,
    pub depth_bias_constant: f32,
    pub depth_bias_clamp: f32,
    pub depth_bias_slope: f32,
    pub line_width: f32,
    pub depth_bounds: u32,
    pub min_depth_bounds: f32,
    pub max_depth_bounds: f32,
    pub sample_mask: u64,
    pub sample_shading: u32,
    pub min_sample_shading: f32,
    pub alpha_to_coverage: u32,
    pub alpha_to_one: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct VertexBinding {
    pub binding: u32,
    pub stride: u32,
    pub rate: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct VertexAttribute {
    pub location: u32,
    pub binding: u32,
    pub format: u32,
    pub offset: u32,
}
#[repr(C)]
pub struct VertexInput {
    pub header: Record,
    pub binding_count: u32,
    pub attribute_count: u32,
    pub bindings: *const VertexBinding,
    pub attributes: *const VertexAttribute,
}
#[repr(C)]
pub struct Attachment {
    pub view: *mut super::View,
    pub resolve_view: *mut super::View,
    pub state: u32,
    pub resolve_state: u32,
    pub load_op: u32,
    pub store_op: u32,
    pub resolve_mode: u32,
    pub clear: ClearValue,
}
#[repr(C)]
pub struct RenderDesc {
    pub header: Record,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub layers: u32,
    pub view_mask: u32,
    pub samples: u32,
    pub flags: u32,
    pub color_count: u32,
    pub colors: *const Attachment,
    pub depth: *const Attachment,
    pub stencil: *const Attachment,
    pub scratch: HostSpan,
}
#[repr(C)]
pub struct ViewportState {
    pub header: Record,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub min_depth: f32,
    pub max_depth: f32,
    pub scissor_x: i32,
    pub scissor_y: i32,
    pub scissor_width: u32,
    pub scissor_height: u32,
}
#[repr(C)]
pub struct DrawDesc {
    pub count: u32,
    pub instances: u32,
    pub first: u32,
    pub first_instance: u32,
    pub vertex_offset: i32,
}
/// GPU-readable indirect draw wire record; no encoding or ownership wrapper.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DrawArguments {
    pub count: u32,
    pub instances: u32,
    pub first: u32,
    pub first_instance: u32,
}
/// Indexed indirect wire order differs from the direct DrawDesc record.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DrawIndexedArguments {
    pub count: u32,
    pub instances: u32,
    pub first: u32,
    pub vertex_offset: i32,
    pub first_instance: u32,
}
#[repr(C)]
pub struct Indirect {
    pub arguments: Span,
    pub stride: u32,
    pub maximum_count: u32,
    pub count: Span,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ExecutionLimits {
    pub max_inline_bytes: u64,
    pub max_groups: [u32; 3],
    pub max_local_size: [u32; 3],
    pub max_local_invocations: u32,
    pub max_shared_memory: u32,
    pub argument_kinds: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Bytes {
    pub data: *const c_void,
    pub size: usize,
}
// Prepared arguments: one byte namespace and explicit native resource roles.
pub const ARGUMENT_INLINE: u32 = 0;
pub const ARGUMENT_UNIFORM_ADDRESS: u32 = 1;
pub const ARGUMENT_READ_ADDRESS: u32 = 2;
pub const ARGUMENT_READ_WRITE_ADDRESS: u32 = 3;
pub const ARGUMENT_COMPUTE: u32 = 0;
pub const ARGUMENT_GRAPHICS: u32 = 1;
pub const ARGUMENT_DIRECT: u32 = u32::MAX;
pub const ARGUMENT_BIND_PRESERVES: u32 = 0;
pub const ARGUMENT_BIND_INVALIDATES_ON_CHANGE: u32 = 1;
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ArgumentField {
    pub offset: u32,
    pub size: u32,
    pub kind: u32,
    pub address_alignment: u32,
    pub stages: u64,
    pub space: u32,
    pub binding: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ArgumentLayoutDesc {
    pub execution_class: u32,
    pub byte_size: u32,
    pub field_count: u32,
    pub flags: u32,
    pub fields: *const ArgumentField,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArgumentStateInfo {
    pub domain: u64,
    pub binding_key: u64,
    pub offset_base: u32,
    pub byte_size: u32,
    pub bind_rule: u32,
    pub reserved: u32,
}
#[repr(C)]
pub struct ShaderRequirements {
    pub header: Record,
    pub features: Features,
    pub local_size: [u32; 3],
    pub shared_memory: u32,
}
#[repr(C)]
pub struct SpecializationEntry {
    pub id: u32,
    pub offset: u32,
    pub size: u64,
}
#[repr(C)]
pub struct Specialization {
    pub header: Record,
    pub count: u32,
    pub reserved: u32,
    pub entries: *const SpecializationEntry,
    pub data: Bytes,
}
#[repr(C)]
pub struct Shader {
    pub stage: u32,
    pub format: u32,
    pub code: Bytes,
    pub entry: *const c_char,
    pub specialization: *const Record,
    pub subgroup: *const Record,
}
#[repr(C)]
pub struct ExecutableDesc {
    pub header: Record,
    pub kind: u32,
    pub shader_count: u32,
    pub shaders: *const Shader,
    pub static_state: *const Record,
    pub dynamic_state: u64,
    pub requirements: *const Record,
    pub cache: *mut super::ExecutableCache,
    pub compile_flags: u32,
    pub reserved: u32,
}
#[repr(C)]
pub struct Launch {
    pub groups: Extent,
    pub dynamic_shared_bytes: u32,
    pub extensions: *const Record,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct DescriptorLimits {
    pub max_resource_capacity: u64,
    pub max_sampler_capacity: u64,
    pub uniform_address_alignment: u64,
    pub storage_address_alignment: u64,
    pub max_uniform_range: u64,
    pub max_storage_range: u64,
    pub max_sampler_lod_bias: f32,
    pub max_sampler_anisotropy: f32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct HostSpan {
    pub data: *mut c_void,
    pub size: u64,
}
#[repr(C)]
pub struct ResourceDescriptor {
    pub kind: u32,
    pub image_state: u32,
    pub view: *mut super::View,
    pub buffer: Span,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SamplerDesc {
    pub header: Record,
    pub min_filter: u32,
    pub mag_filter: u32,
    pub mip_filter: u32,
    pub address_mode: [u32; 3],
    pub compare_enable: u32,
    pub compare_op: u32,
    pub border_color: u32,
    pub unnormalized_coordinates: u32,
    pub min_lod: f32,
    pub max_lod: f32,
    pub lod_bias: f32,
    pub max_anisotropy: f32,
}
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
pub struct ImageTransfer {
    pub source: ImageRegion,
    pub destination: ImageRegion,
    pub source_state: u32,
    pub destination_state: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BlitRegion {
    pub aspect: u32,
    pub mip: u32,
    pub first_layer: u32,
    pub layer_count: u32,
    pub offsets: [Offset; 2],
}
#[repr(C)]
pub struct ImageBlit {
    pub source: BlitRegion,
    pub destination: BlitRegion,
    pub source_state: u32,
    pub destination_state: u32,
    pub filter: u32,
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

pub const MEMORY_LIMITS: u32 = 6;
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
    pub available: Features,
    pub enabled: Features,
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
    pub required_features: Features,
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
    pub level: u32,
    pub reserved: u32,
}
#[repr(C)]
pub struct Inheritance {
    pub header: Record,
    pub occlusion: u32,
    pub query_flags: u32,
    pub statistics: u64,
    pub rendering: *const Record,
    pub resource_heap: *mut super::DescriptorHeap,
    pub sampler_heap: *mut super::DescriptorHeap,
}
#[repr(C)]
pub struct RenderInheritance {
    pub header: Record,
    pub color_count: u32,
    pub samples: u32,
    pub view_mask: u32,
    pub reserved: u32,
    pub depth_format: u32,
    pub stencil_format: u32,
    pub color_formats: *const u32,
    pub scratch: HostSpan,
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
pub const SUBMIT_REJECTED: u32 = 0;
pub const SUBMIT_ACCEPTED: u32 = 1;
pub const SUBMIT_UNKNOWN: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SubmitDesc {
    pub list_count: u32,
    pub wait_count: u32,
    pub signal_count: u32,
    pub binary_wait_count: u32,
    pub binary_signal_count: u32,
    pub flags: u32,
    pub lists: *const *mut super::List,
    pub waits: *const SyncPoint,
    pub signals: *const SyncPoint,
    pub binary_waits: *const BinaryPoint,
    pub binary_signals: *const BinaryPoint,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubmitOutcome {
    pub status: Status,
    pub acceptance: u32,
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

#[repr(C)]
#[derive(Clone, Copy)]
pub struct HeapRequest {
    pub kind: u32,
    pub storage: u32,
    pub usage: u32,
    pub flags: u32,
    pub capacity: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeapRequirements {
    pub unit_bytes: u64,
    pub backing_size: u64,
    pub backing_alignment: u64,
    pub reserved_size: u64,
    pub reserved_alignment: u64,
    pub memory_usage: u64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct HeapDesc {
    pub request: HeapRequest,
    pub memory: Span,
    pub host: HostSpan,
    pub reserved_offset: u64,
    pub reserved_size: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DescriptorShape {
    pub size: u64,
    pub alignment: u64,
    pub array_stride: u64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct HeapLocation {
    pub heap: *mut super::DescriptorHeap,
    pub offset: u64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DescriptorCopy {
    pub source: HeapLocation,
    pub destination: HeapLocation,
    pub size: u64,
}

// Per-entry dependencies and immutable enabled-domain capabilities.
pub const DEPENDENCY_LIMITS: u32 = 19;
pub const MEMORY_SCOPE_NONE: u32 = 0;
pub const MEMORY_SCOPE_OBJECT: u32 = 1;
pub const DEPENDENCY_SPLIT: u32 = 1;
pub const DEPENDENCY_OWNERSHIP: u32 = 2;
pub const DEPENDENCY_BINARY: u32 = 4;
pub const MEMORY_SCOPE_BYTES: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
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
#[derive(Clone, Copy, Default)]
pub struct BarrierScope {
    pub stages_before: u64,
    pub stages_after: u64,
    pub access_before: u64,
    pub access_after: u64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MemoryBarrier {
    pub scope: BarrierScope,
    pub range: Span,
    pub source_domain: u32,
    pub destination_domain: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ImageBarrier {
    pub scope: BarrierScope,
    pub image: *mut super::Image,
    pub range: Subresources,
    pub old_state: u32,
    pub new_state: u32,
    pub source_domain: u32,
    pub destination_domain: u32,
    pub discard: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
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
#[derive(Clone, Copy)]
pub struct SplitDependency {
    pub split: *const super::Split,
    pub dependency: *const Dependency,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VertexBufferBinding {
    pub span: Span,
    pub stride: u32,
    pub flags: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GeneratedVertexBuffer {
    pub address: u64,
    pub size: u32,
    pub stride: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GeneratedIndexBuffer {
    pub address: u64,
    pub size: u32,
    pub index_type: u32,
}
