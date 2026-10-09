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
pub const SAMPLER_DESC: u32 = 108;
pub const HEAP_BINDING: u32 = 109;
pub const DESCRIPTOR_LIMITS: u32 = 7;
pub const SAMPLER_ANISOTROPY: u64 = 8;
pub const EXECUTION_LIMITS: u32 = 8;
pub const EXECUTABLE_DESC: u32 = 110;
pub const ARGUMENT_INTERFACE: u32 = 111;
pub const SHADER_REQUIREMENTS: u32 = 112;
pub const SPECIALIZATION: u32 = 113;
pub const GRAPHICS_STATE: u32 = 114;
pub const RENDER_DESC: u32 = 115;
pub const VIEWPORT_STATE: u32 = 116;
pub const GRAPHICS_LIMITS: u32 = 9;

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
    pub max_inline_size: u64,
    pub max_groups: [u32; 3],
    pub max_local_size: [u32; 3],
    pub max_local_invocations: u32,
    pub max_shared_memory: u32,
    pub argument_flags: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Bytes {
    pub data: *const c_void,
    pub size: usize,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RootSlot {
    pub stages: u64,
    pub offset: u32,
    pub alignment: u32,
}
#[repr(C)]
pub struct ArgumentInterface {
    pub header: Record,
    pub byte_size: u32,
    pub root_count: u32,
    pub roots: *const RootSlot,
}
#[repr(C)]
pub struct ShaderRequirements {
    pub header: Record,
    pub features: u64,
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
    pub interface_metadata: *const Record,
    pub specialization: *const Record,
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
    pub native_cache: Bytes,
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
    pub buffer_size: u64,
    pub buffer_alignment: u64,
    pub image_size: u64,
    pub image_alignment: u64,
    pub sampler_size: u64,
    pub sampler_alignment: u64,
    pub resource_heap_alignment: u64,
    pub resource_heap_max_size: u64,
    pub resource_reserved_size: u64,
    pub resource_reserved_alignment: u64,
    pub sampler_heap_alignment: u64,
    pub sampler_heap_max_size: u64,
    pub sampler_reserved_size: u64,
    pub sampler_reserved_alignment: u64,
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
pub struct HeapBinding {
    pub header: Record,
    pub kind: u32,
    pub flags: u32,
    pub storage: Span,
    pub reserved_offset: u64,
    pub reserved_size: u64,
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
