//! D3D12 foundation: explicit heaps/resources, command-allocator arenas and
//! fence timelines. No hidden copies, staging, host command expansion or waits.
//!
//! D1 scope (branch `dx12-backend`): discovery, requested queues, fence/timeline
//! operations, explicit memory/images, copy/readback, arenas/lists, dependencies
//! and submission errors. Executable/descriptor/graphics/query/generated and
//! presentation surfaces return UNSUPPORTED until their native mapping profiles
//! are resolved in D2/D3; no emulation hides those gaps.
//!
//! COM ownership: every native interface value held here owns exactly one
//! reference (windows-core releases on drop). Cold construction may query and
//! create; ordinary recording/submission never calls AddRef/Release or allocates
//! beyond caller-provided scratch and the arena's explicit slot storage.
use super::types::*;
use super::{Adapter, Discovery, Snapshot};
use std::{
    mem::size_of,
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};
use windows::core::Interface;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_12_0;
use windows::Win32::Graphics::Direct3D12 as d3d;
use windows::Win32::Graphics::Dxgi as dxgi;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN;
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject, INFINITE};

#[path = "commands.rs"]
mod commands;
#[path = "descriptors.rs"]
mod descriptors;
#[path = "rendering.rs"]
mod rendering;
#[path = "executables.rs"]
mod executables;
#[path = "formats.rs"]
pub(in crate::foundation) mod formats;
#[path = "graphics.rs"]
mod graphics;
#[path = "images.rs"]
mod images;
#[path = "cache.rs"]
mod cache;
#[path = "memory.rs"]
mod memory;
#[path = "queries.rs"]
mod queries;
#[path = "stubs.rs"]
mod stubs;

pub use commands::{Arena, List};
pub use images::{Image, View};
pub use memory::Memory;
pub use executables::Executable;
pub use cache::ExecutableCache;
pub use queries::QueryPool;
pub use crate::foundation::presentation::Swapchain;
pub use stubs::{Binary, Fence, GeneratedLayout, GeneratedSet, Split};

pub(super) use commands::{barrier_scratch, submit_batch_scratch, submit_scratch};
pub(super) use descriptors::descriptor_scratch;
pub(super) use commands::vertex_scratch;
pub(super) use rendering::render_scratch;
pub(super) use stubs::{
    execute_scratch, executable_record, inheritance_scratch, present_scratch, split_scratch,
};

/// Queue domains of this adapter profile: direct, compute, copy queues.
const DOMAIN_GRAPHICS: u32 = 0;
const DOMAIN_COMPUTE: u32 = 1;
const DOMAIN_COPY: u32 = 2;
/// Explicit software bound on concurrently requestable queues per domain; D3D12
/// creates command queues on demand and exposes no physical queue-family count.
const DOMAIN_QUEUE_COUNT: u32 = 16;

/// Immutable D3D12 discovery state retained by each shared `Adapter`.
pub(super) struct AdapterState {
    pub(super) adapter: dxgi::IDXGIAdapter1,
    /// Cached physical facts needed at device creation; all queried at discovery.
    pub(super) baseline_supported: bool,
    pub(super) heap_tier: u32,
}

/// Headless discovery over the DXGI adapter list. No window-system profile.
pub(super) fn discover() -> Result<Box<Discovery>, Status> {
    // SAFETY: factory creation has no inputs; owned output adopted below.
    let factory: dxgi::IDXGIFactory2 =
        unsafe { dxgi::CreateDXGIFactory2(dxgi::DXGI_CREATE_FACTORY_FLAGS(0)) }
            .map_err(map_error)?;
    discover_factory(&factory)
}

/// Presentation-enabled discovery is not part of the D1 D3D12 profile: DXGI
/// swapchains need an explicit external-window surface record (D3).
pub(super) fn discover_presentation() -> Result<Box<Discovery>, Status> {
    Err(UNSUPPORTED)
}

fn discover_factory(factory: &dxgi::IDXGIFactory2) -> Result<Box<Discovery>, Status> {
    let mut adapters = Vec::new();
    for index in 0.. {
        // SAFETY: enumeration by index; the returned adapter adopts one reference.
        let adapter: dxgi::IDXGIAdapter1 = match unsafe {
            factory.EnumAdapters1(index)
        } {
            Ok(adapter) => adapter,
            Err(_) => break,
        };
        let state = probe(adapter)?;
        adapters.push(Adapter {
            snapshot: state.0,
            state: state.1,
        });
    }
    Ok(Box::new(Discovery { adapters }))
}

/// Build the immutable snapshot and cached profile of one adapter. Adapters that
/// cannot produce any D3D12 device remain listed with `baseline_supported = 0`.
fn probe(adapter: dxgi::IDXGIAdapter1) -> Result<(Snapshot, AdapterState), Status> {
    // SAFETY: cold discovery path; the temporary device is dropped after probing.
    let mut device: Option<d3d::ID3D12Device> = None;
    let created = unsafe {
        windows::Win32::Graphics::Direct3D12::D3D12CreateDevice(
            &adapter,
            D3D_FEATURE_LEVEL_12_0,
            &mut device,
        )
        .map_err(map_error)
    };
    let Some(device) = device.filter(|_| created.is_ok()) else {
        // SAFETY: adapter stays live; the description is returned by value.
        let info = unsafe { adapter.GetDesc1() }.map_err(map_error)?;
        return Ok((
            Snapshot {
                cache_uuid: adapter_uuid(&info),
                info: adapter_info(&info, 0),
                ..empty_snapshot()
            },
            AdapterState {
                adapter,
                baseline_supported: false,
                heap_tier: 1,
            },
        ));
    };
    // SAFETY: adapter stays live; the description is returned by value.
    let info = unsafe { adapter.GetDesc1() }.map_err(map_error)?;

    let mut options = d3d::D3D12_FEATURE_DATA_D3D12_OPTIONS::default();
    // SAFETY: feature query structs are sized by their compile-time types.
    unsafe {
        device
            .CheckFeatureSupport(
                d3d::D3D12_FEATURE_D3D12_OPTIONS,
                ptr::from_mut(&mut options).cast(),
                size_of::<d3d::D3D12_FEATURE_DATA_D3D12_OPTIONS>() as u32,
            )
            .map_err(map_error)?
    };
    let mut options12 = d3d::D3D12_FEATURE_DATA_D3D12_OPTIONS12::default();
    let enhanced_barriers = unsafe {
        device
            .CheckFeatureSupport(
                d3d::D3D12_FEATURE_D3D12_OPTIONS12,
                ptr::from_mut(&mut options12).cast(),
                size_of::<d3d::D3D12_FEATURE_DATA_D3D12_OPTIONS12>() as u32,
            )
            .is_ok()
            && options12.EnhancedBarriersSupported.as_bool()
    };
    let levels = [D3D_FEATURE_LEVEL_12_0];
    let mut level_query = d3d::D3D12_FEATURE_DATA_FEATURE_LEVELS {
        NumFeatureLevels: levels.len() as u32,
        pFeatureLevelsRequested: levels.as_ptr(),
        ..Default::default()
    };
    // SAFETY: the requested-level array outlives the call.
    let feature_level = unsafe {
        device
            .CheckFeatureSupport(
                d3d::D3D12_FEATURE_FEATURE_LEVELS,
                ptr::from_mut(&mut level_query).cast(),
                size_of::<d3d::D3D12_FEATURE_DATA_FEATURE_LEVELS>() as u32,
            )
            .map_err(map_error)?;
        level_query.MaxSupportedFeatureLevel
    };
    // A baseline requires a command-list level that can record enhanced barriers.
    let command_list_barriers = command_list_barriers(&device).unwrap_or(false);
    let baseline = enhanced_barriers && command_list_barriers;

    let mut snapshot = empty_snapshot();
    snapshot.cache_uuid = adapter_uuid(&info);
    snapshot.info = adapter_info(&info, feature_level.0 as u32);
    snapshot.queues = queue_domains();
    let (memory_types, memory_heaps) = memory_profile(&device, &adapter, &info);
    snapshot.memory_types = memory_types;
    snapshot.memory_heaps = memory_heaps;
    snapshot.memory_budget_supported = adapter.cast::<dxgi::IDXGIAdapter3>().is_ok();
    snapshot.memory_limits = memory_limits(&device);
    snapshot.descriptor_limits = descriptors::descriptor_limits();
    snapshot.execution_limits = execution_limits();
    snapshot.formats = formats::format_snapshot(&device);
    snapshot.features = FeatureInfo {
        // Physically supported optional features on this backend: exact
        // occlusion, the native pipeline-statistics counters, and explicit
        // executable-cache synchronization control.
        available: RASTER | PIPELINE_STATISTICS | PRECISE_OCCLUSION | CACHE_CONTROL,
        enabled: 0,
        max_timeline_difference: u64::MAX,
        baseline_supported: u32::from(baseline),
        device_scope: 0,
    };
    Ok((
        snapshot,
        AdapterState {
            adapter,
            baseline_supported: baseline,
            heap_tier: options.ResourceHeapTier.0 as u32,
        },
    ))
}

/// Probe whether command lists expose the enhanced-barrier entry points that the
/// dependency profile requires. Cold path; the temporary objects are released.
fn command_list_barriers(device: &d3d::ID3D12Device) -> Result<bool, Status> {
    // SAFETY: creation outputs are owned by the returned values and drop normally.
    let allocator: d3d::ID3D12CommandAllocator = unsafe {
        device
            .CreateCommandAllocator(d3d::D3D12_COMMAND_LIST_TYPE_DIRECT)
            .map_err(map_error)?
    };
    let list: d3d::ID3D12GraphicsCommandList = unsafe {
        device
            .CreateCommandList(0, d3d::D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None)
            .map_err(map_error)?
    };
    // SAFETY: query returns an owned reference or an error; adoption is automatic.
    let result = list.cast::<d3d::ID3D12GraphicsCommandList7>();
    drop(list);
    drop(allocator);
    Ok(result.is_ok())
}

fn empty_snapshot() -> Snapshot {
    Snapshot {
        cache_uuid: [0; 16],
        info: AdapterInfo {
            name: [0; 256],
            backend: crate::BACKEND_D3D12,
            vendor_id: 0,
            device_id: 0,
            device_type: 0,
            native_api_version: 0,
        },
        queues: Vec::new(),
        memory_types: Vec::new(),
        memory_heaps: Vec::new(),
        memory_budget_supported: false,
        memory_limits: MemoryLimits::default(),
        descriptor_limits: DescriptorLimits::default(),
        execution_limits: ExecutionLimits::default(),
        graphics_limits: GraphicsLimits {
            // ExecuteIndirect command counts are u32 with no lower native cap.
            max_indirect_count: u32::MAX,
            ..Default::default()
        },
        query_limits: QueryLimits {
            // All eleven native pipeline-statistics counters are physical.
            statistics: (1 << 11) - 1,
            // D3D12 timestamps work on every queue type.
            timestamp_compute_graphics: 1,
            timestamp_period_ns: 0.0,
        },
        subgroup_limits: SubgroupLimits::default(),
        float_controls: FloatControls::default(),
        matrix_tuples: Vec::new(),
        integer_dot: IntegerDotInfo::default(),
        mesh_limits: MeshLimits::default(),
        generated_limits: GeneratedLimits::default(),
        formats: Vec::new(),
        features: FeatureInfo::default(),
    }
}

/// Cache identity derived from the adapter LUID and device identity. D3D12 has
/// no native pipeline-cache UUID; this identifies this adapter for cache bytes.
fn adapter_uuid(info: &dxgi::DXGI_ADAPTER_DESC1) -> [u8; 16] {
    let mut uuid = [0u8; 16];
    let luid = (i64::from(info.AdapterLuid.HighPart) << 32) | i64::from(info.AdapterLuid.LowPart);
    uuid[..8].copy_from_slice(&luid.to_le_bytes());
    uuid[8..12].copy_from_slice(&info.VendorId.to_le_bytes());
    uuid[12..].copy_from_slice(&info.DeviceId.to_le_bytes());
    uuid
}

fn adapter_info(info: &dxgi::DXGI_ADAPTER_DESC1, feature_level: u32) -> AdapterInfo {
    let mut name = [0 as std::ffi::c_char; 256];
    for (slot, wide) in name.iter_mut().zip(info.Description.iter()) {
        // Lossy byte copy of the UTF-16 description; terminated at the first NUL.
        if *wide == 0 {
            break;
        }
        *slot = *wide as u8 as std::ffi::c_char;
    }
    // device_type reuses the established vocabulary: 1 integrated, 2 discrete,
    // 4 CPU-like. DXGI exposes only software flags and dedicated memory sizes.
    let device_type = if info.Flags & dxgi::DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
        4
    } else if info.DedicatedVideoMemory != 0 {
        2
    } else {
        1
    };
    // native_api_version packs (major << 22) | (minor << 12); D3D12 reports the
    // maximum supported feature level as major.minor (e.g. 12.1).
    let major = (feature_level >> 12) & 0xf;
    let minor = (feature_level >> 4) & 0xf;
    let native_api_version = major << 22 | minor << 12;
    AdapterInfo {
        name,
        backend: crate::BACKEND_D3D12,
        vendor_id: info.VendorId,
        device_id: info.DeviceId,
        device_type,
        native_api_version,
    }
}

fn queue_domains() -> Vec<QueueInfo> {
    [
        (DOMAIN_GRAPHICS, GRAPHICS | COMPUTE | TRANSFER, 64),
        (DOMAIN_COMPUTE, COMPUTE | TRANSFER, 64),
        (DOMAIN_COPY, TRANSFER, 0),
    ]
    .into_iter()
    .map(|(domain, flags, timestamp_bits)| QueueInfo {
        domain,
        flags,
        count: DOMAIN_QUEUE_COUNT,
        timestamp_bits,
        copy_granularity: [1, 1, 1],
        reserved: 0,
    })
    .collect()
}

/// Memory types are the three standard D3D12 heap types; each reports the
/// physical heap (native memory pool) it is served from. Only pools that some
/// standard heap type actually maps to appear as physical heaps: a UMA or
/// software adapter may have a single heap.
fn memory_profile(
    device: &d3d::ID3D12Device,
    adapter: &dxgi::IDXGIAdapter1,
    info: &dxgi::DXGI_ADAPTER_DESC1,
) -> (Vec<MemoryTypeInfo>, Vec<MemoryHeapInfo>) {
    let standard = [
        (0u32, d3d::D3D12_HEAP_TYPE_DEFAULT),
        (
            MEMORY_HOST_VISIBLE | MEMORY_COHERENT,
            d3d::D3D12_HEAP_TYPE_UPLOAD,
        ),
        (
            MEMORY_HOST_VISIBLE | MEMORY_COHERENT | 8,
            d3d::D3D12_HEAP_TYPE_READBACK,
        ),
    ];
    // Pools in native order: L0 (local) before L1 (non-local).
    let mut pools: Vec<d3d::D3D12_MEMORY_POOL> = Vec::new();
    let mut type_pools = Vec::new();
    for (_, ty) in standard {
        // SAFETY: value-returning query; no object creation or mutation.
        let pool = unsafe { device.GetCustomHeapProperties(0, ty) }.MemoryPoolPreference;
        if !pools.contains(&pool) {
            pools.push(pool);
        }
        type_pools.push(pool);
    }
    pools.sort_by_key(|p| p.0);
    let types = standard
        .into_iter()
        .zip(type_pools)
        .enumerate()
        .map(|(id, ((properties, _), pool))| MemoryTypeInfo {
            id: id as u32,
            heap: pools.iter().position(|p| *p == pool).unwrap_or(0) as u32,
            properties,
            reserved: 0,
        })
        .collect();
    let heaps = pools
        .into_iter()
        .enumerate()
        .map(|(id, pool)| {
            let local = pool != d3d::D3D12_MEMORY_POOL_L1;
            let group = if local {
                dxgi::DXGI_MEMORY_SEGMENT_GROUP_LOCAL
            } else {
                dxgi::DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL
            };
            let dedicated = info.DedicatedVideoMemory as u64;
            let shared = info.SharedSystemMemory as u64;
            // DXGI exposes no exact physical segment size: discrete adapters
            // report dedicated memory for the local pool, UMA and software
            // adapters back it with system memory. The discovery-time budget is
            // a lower bound of the physical size and keeps later budget queries
            // bounded by the reported heap size.
            let mut size = if local && dedicated != 0 {
                dedicated
            } else {
                shared
            };
            // SAFETY: adapter stays live; the query only fills the out-structure.
            if let Ok(adapter3) = adapter.cast::<dxgi::IDXGIAdapter3>() {
                let mut budget = dxgi::DXGI_QUERY_VIDEO_MEMORY_INFO::default();
                if unsafe { adapter3.QueryVideoMemoryInfo(0, group, &mut budget) }.is_ok() {
                    size = size.max(budget.Budget);
                }
            }
            MemoryHeapInfo {
                size,
                id: id as u32,
                device_local: u32::from(local),
            }
        })
        .collect();
    (types, heaps)
}

/// Buffer-size and allocation bounds are probed, not taken from documentation:
/// `GetResourceAllocationInfo` reports a valid allocation for supported sizes and
/// an all-ones result beyond native limits.
fn memory_limits(device: &d3d::ID3D12Device) -> MemoryLimits {
    let mut max = 0;
    for exponent in (27..=40).rev() {
        let size = 1u64 << exponent;
        let desc = d3d::D3D12_RESOURCE_DESC {
            Dimension: d3d::D3D12_RESOURCE_DIMENSION_BUFFER,
            Width: size,
            Height: 1,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: DXGI_FORMAT_UNKNOWN,
            SampleDesc: dxgi::Common::DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Layout: d3d::D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            Flags: d3d::D3D12_RESOURCE_FLAG_NONE,
            Alignment: 0,
        };
        // SAFETY: query with a single well-formed description; no object creation.
        let info = unsafe { device.GetResourceAllocationInfo(0, std::slice::from_ref(&desc)) };
        if info.SizeInBytes != u64::MAX && info.Alignment != 0 {
            max = size;
            break;
        }
    }
    MemoryLimits {
        max_buffer_size: max,
        max_allocation_size: max,
        // All CPU-visible D3D12 memory is coherent; no atom round-out is needed.
        cache_atom_size: 1,
        // Conservative documented floor: small placed resources align to 4 KiB.
        map_alignment: 4096,
    }
}

/// Physical execution constants of the D3D12 compute profile. The argument
/// namespace is root constants: `max_inline_size` is the largest namespace with
/// no root descriptors (63 dwords); composition with root descriptors is
/// checked per executable against the 64-dword root budget. The reported
/// argument profile is the provisional ROOT_DESCRIPTORS profile documented in
/// docs/dx12-arguments-profile.md.
fn execution_limits() -> ExecutionLimits {
    ExecutionLimits {
        max_inline_size: 252,
        max_groups: [65535, 65535, 65535],
        max_local_size: [1024, 1024, 64],
        max_local_invocations: 1024,
        max_shared_memory: 32768,
        argument_flags: ARGUMENT_SHARED_BYTES | ARGUMENT_ROOT_DESCRIPTORS,
    }
}

/// Map ordinary native failures onto foundation statuses without poisoning.
/// Cold-path creation and query failures use this mapping.
pub(super) fn map_error(error: windows::core::Error) -> Status {
    use windows::core::HRESULT;
    let code = error.code();
    eprintln!("DX12PROBE map_error {:#x}", code.0 as u32);
    if code == HRESULT(0x8007_000E_u32 as i32) /* E_OUTOFMEMORY */ {
        OUT_OF_MEMORY
    } else if matches!(
        code,
        windows::Win32::Graphics::Dxgi::DXGI_ERROR_DEVICE_REMOVED
            | windows::Win32::Graphics::Dxgi::DXGI_ERROR_DEVICE_HUNG
            | windows::Win32::Graphics::Dxgi::DXGI_ERROR_DEVICE_RESET
            | windows::Win32::Graphics::Dxgi::DXGI_ERROR_DRIVER_INTERNAL_ERROR
    ) {
        DEVICE_LOST
    } else {
        BACKEND_ERROR
    }
}

pub struct Device {
    pub(super) snapshot: Snapshot,
    pub(super) device: d3d::ID3D12Device,
    /// Device-level dispatch command signature for indirect dispatch.
    pub(super) dispatch_signature: d3d::ID3D12CommandSignature,
    /// Device-level draw command signatures for indirect draws. The native
    /// command stride equals the wire record size; larger argument strides
    /// have no native form and stay visible.
    pub(super) draw_signature: d3d::ID3D12CommandSignature,
    pub(super) draw_indexed_signature: d3d::ID3D12CommandSignature,
    pub(super) heap_tier: u32,
    /// Retained for the live budget query; no other adapter dependency exists.
    adapter: dxgi::IDXGIAdapter1,
    /// Native render/depth target descriptor heaps; attachment views own slots.
    view_heaps: ViewHeaps,
    queues: Vec<Queue>,
    lost: AtomicBool,
}

/// Descriptor heaps for render/depth attachment views. Slot allocation is a
/// cold-path counter; recording performs no allocation.
pub(super) struct ViewHeaps {
    rtv: d3d::ID3D12DescriptorHeap,
    dsv: d3d::ID3D12DescriptorHeap,
    rtv_next: std::sync::atomic::AtomicU32,
    dsv_next: std::sync::atomic::AtomicU32,
}

pub struct Queue {
    pub(super) handle: d3d::ID3D12CommandQueue,
    device: *const Device,
    domain: u32,
    index: u32,
}

// SAFETY: device data and queue handles are immutable after construction; the
// only mutated state is the atomic loss flag. Per-queue host access and
// destruction are externally synchronized by the C/Rust contract.
unsafe impl Send for Device {}
unsafe impl Sync for Device {}

impl Device {
    /// The native command queue of one recorded domain (swapchain binding).
    pub(in crate::foundation) fn command_queue(
        &self,
        domain: u32,
    ) -> Option<&d3d::ID3D12CommandQueue> {
        self.queues
            .iter()
            .find(|queue| queue.domain == domain)
            .map(|queue| &queue.handle)
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        // Handles drop their COM references; the caller has retired all children.
    }
}

impl Device {
    pub(super) fn create(
        adapter: &Adapter,
        requests: &[QueueRequest],
        enabled: u64,
    ) -> Result<Box<Self>, Status> {
        if !adapter.state.baseline_supported {
            return Err(UNSUPPORTED);
        }
        let mut snapshot = adapter.snapshot.clone();
        snapshot.features.enabled = enabled;
        snapshot.features.device_scope = 1;
        // Device queries report created queue counts, not physical capacity.
        for q in &mut snapshot.queues {
            q.count = requests
                .iter()
                .find(|r| r.domain == q.domain)
                .map_or(0, |r| r.count);
        }
        // SAFETY: creation adopts one owned device reference.
        let mut created: Option<d3d::ID3D12Device> = None;
        unsafe {
            windows::Win32::Graphics::Direct3D12::D3D12CreateDevice(
                &adapter.state.adapter,
                D3D_FEATURE_LEVEL_12_0,
                &mut created,
            )
            .map_err(map_error)?
        };
        let device = created.ok_or(BACKEND_ERROR)?;
        let mut queues = Vec::with_capacity(requests.len());
        for request in requests {
            let ty = match request.domain {
                DOMAIN_GRAPHICS => d3d::D3D12_COMMAND_LIST_TYPE_DIRECT,
                DOMAIN_COMPUTE => d3d::D3D12_COMMAND_LIST_TYPE_COMPUTE,
                DOMAIN_COPY => d3d::D3D12_COMMAND_LIST_TYPE_COPY,
                _ => return Err(INVALID),
            };
            // D3D12 exposes two ordinary priority classes: 1.0 selects HIGH,
            // every other finite priority in [0,1) selects NORMAL.
            let priority = if request.priority == 1.0 {
                d3d::D3D12_COMMAND_QUEUE_PRIORITY_HIGH
            } else {
                d3d::D3D12_COMMAND_QUEUE_PRIORITY_NORMAL
            };
            let desc = d3d::D3D12_COMMAND_QUEUE_DESC {
                Type: ty,
                Priority: priority.0,
                Flags: d3d::D3D12_COMMAND_QUEUE_FLAG_NONE,
                NodeMask: 0,
            };
            for index in 0..request.count {
                // SAFETY: well-formed queue description; output reference adopted.
                let handle = unsafe {
                    device
                        .CreateCommandQueue::<d3d::ID3D12CommandQueue>(&desc)
                        .map_err(map_error)?
                };
                queues.push(Queue {
                    handle,
                    device: ptr::null(),
                    domain: request.domain,
                    index,
                });
            }
        }
        // Indirect dispatch signature: one DISPATCH argument, created once.
        let argument = d3d::D3D12_INDIRECT_ARGUMENT_DESC {
            Type: d3d::D3D12_INDIRECT_ARGUMENT_TYPE_DISPATCH,
            // The dispatch argument carries no inline payload.
            Anonymous: unsafe { core::mem::zeroed() },
        };
        let signature_desc = d3d::D3D12_COMMAND_SIGNATURE_DESC {
            ByteStride: size_of::<d3d::D3D12_DISPATCH_ARGUMENTS>() as u32,
            NumArgumentDescs: 1,
            pArgumentDescs: &argument,
            NodeMask: 0,
        };
        // SAFETY: self-contained description; output signature is adopted.
        let mut dispatch_signature: Option<d3d::ID3D12CommandSignature> = None;
        unsafe {
            device
                .CreateCommandSignature(
                    &signature_desc,
                    None::<&d3d::ID3D12RootSignature>,
                    &mut dispatch_signature,
                )
                .map_err(map_error)?
        };
        let draw_argument = d3d::D3D12_INDIRECT_ARGUMENT_DESC {
            Type: d3d::D3D12_INDIRECT_ARGUMENT_TYPE_DRAW,
            Anonymous: unsafe { core::mem::zeroed() },
        };
        let draw_desc = d3d::D3D12_COMMAND_SIGNATURE_DESC {
            ByteStride: size_of::<d3d::D3D12_DRAW_ARGUMENTS>() as u32,
            NumArgumentDescs: 1,
            pArgumentDescs: &draw_argument,
            NodeMask: 0,
        };
        // SAFETY: self-contained description; output signature is adopted.
        let mut draw_signature: Option<d3d::ID3D12CommandSignature> = None;
        unsafe {
            device
                .CreateCommandSignature(
                    &draw_desc,
                    None::<&d3d::ID3D12RootSignature>,
                    &mut draw_signature,
                )
                .map_err(map_error)?
        };
        let draw_indexed_argument = d3d::D3D12_INDIRECT_ARGUMENT_DESC {
            Type: d3d::D3D12_INDIRECT_ARGUMENT_TYPE_DRAW_INDEXED,
            Anonymous: unsafe { core::mem::zeroed() },
        };
        let draw_indexed_desc = d3d::D3D12_COMMAND_SIGNATURE_DESC {
            ByteStride: size_of::<d3d::D3D12_DRAW_INDEXED_ARGUMENTS>() as u32,
            NumArgumentDescs: 1,
            pArgumentDescs: &draw_indexed_argument,
            NodeMask: 0,
        };
        // SAFETY: self-contained description; output signature is adopted.
        let mut draw_indexed_signature: Option<d3d::ID3D12CommandSignature> = None;
        unsafe {
            device
                .CreateCommandSignature(
                    &draw_indexed_desc,
                    None::<&d3d::ID3D12RootSignature>,
                    &mut draw_indexed_signature,
                )
                .map_err(map_error)?
        };

        // SAFETY: well-formed heap descriptions; outputs are adopted.
        let view_heaps = ViewHeaps {
            rtv: unsafe {
                create_view_heap(&device, d3d::D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 65536)?
            },
            dsv: unsafe {
                create_view_heap(&device, d3d::D3D12_DESCRIPTOR_HEAP_TYPE_DSV, 8192)?
            },
            rtv_next: std::sync::atomic::AtomicU32::new(0),
            dsv_next: std::sync::atomic::AtomicU32::new(0),
        };
        let mut result = Box::new(Self {
            snapshot,
            device,
            draw_signature: draw_signature.ok_or(BACKEND_ERROR)?,
            draw_indexed_signature: draw_indexed_signature.ok_or(BACKEND_ERROR)?,
            dispatch_signature: dispatch_signature.ok_or(BACKEND_ERROR)?,
            heap_tier: adapter.state.heap_tier,
            adapter: adapter.state.adapter.clone(),
            view_heaps,
            queues,
            lost: AtomicBool::new(false),
        });
        let base: *const Device = &*result;
        for queue in &mut result.queues {
            queue.device = base;
        }
        Ok(result)
    }
    pub(super) fn ready(&self) -> Result<(), Status> {
        if self.lost.load(Ordering::Relaxed) {
            Err(DEVICE_LOST)
        } else {
            Ok(())
        }
    }
    /// Ordinary native call failure mapping: device removal poisons the device,
    /// other failures are local errors and leave the device usable. Submission
    /// has its own conservative poisoning policy in `Queue::submit`.
    pub(super) fn result(&self, hr: windows::core::Result<()>) -> Result<(), Status> {
        match hr {
            Ok(()) => Ok(()),
            Err(error) => {
                let status = map_error(error);
                eprintln!("DX12PROBE result() status {}", status);
                if status == DEVICE_LOST {
                    self.lost.store(true, Ordering::Relaxed);
                }
                Err(status)
            }
        }
    }
    /// Conservative submission failure policy: ordinary native submission
    /// errors poison the device (BACKEND_ERROR now, DEVICE_LOST after).
    pub(super) fn submission_result(&self, hr: windows::core::Result<()>) -> Result<(), Status> {
        match hr {
            Ok(()) => Ok(()),
            Err(error) => {
                let status = map_error(error);
                eprintln!("DX12PROBE submission_result() status {}", status);
                if status != OUT_OF_MEMORY {
                    self.lost.store(true, Ordering::Relaxed);
                }
                Err(status)
            }
        }
    }
    /// Allocate one attachment-view descriptor slot of the given native heap
    /// type. Cold path: view creation, never recording.
    pub(super) unsafe fn next_view_slot(
        &self,
        kind: d3d::D3D12_DESCRIPTOR_HEAP_TYPE,
    ) -> Result<d3d::D3D12_CPU_DESCRIPTOR_HANDLE, Status> {
        let (heap, counter, capacity) = match kind {
            d3d::D3D12_DESCRIPTOR_HEAP_TYPE_RTV => (
                &self.view_heaps.rtv,
                &self.view_heaps.rtv_next,
                65536,
            ),
            _ => (
                &self.view_heaps.dsv,
                &self.view_heaps.dsv_next,
                8192,
            ),
        };
        let slot = counter.fetch_add(1, Ordering::Relaxed);
        if slot >= capacity {
            return Err(CAPACITY);
        }
        // SAFETY: live heap; the handle targets one owned slot.
        let mut handle = unsafe { heap.GetCPUDescriptorHandleForHeapStart() };
        let stride =
            // SAFETY: live device query; returns the native slot size.
            unsafe { self.device.GetDescriptorHandleIncrementSize(kind) };
        handle.ptr += stride as usize * slot as usize;
        Ok(handle)
    }

    pub(super) fn queue(&self, domain: u32, index: u32) -> Option<&Queue> {
        self.queues
            .iter()
            .find(|q| q.domain == domain && q.index == index)
    }

    /// Live per-heap budget estimates via DXGI video-memory queries. The
    /// count/capacity protocol matches the shared contract exactly; UNSUPPORTED
    /// preserves outputs when the native query is unavailable.
    /// # Safety
    /// Query storage is writable, aligned and disjoint with its declared capacity.
    pub(in crate::foundation) unsafe fn memory_budget_query(
        &self,
        output: *mut Query,
    ) -> Result<(), Status> {
        self.ready()?;
        if output.is_null() {
            return Err(INVALID);
        }
        // SAFETY: caller guarantees a readable Record header and writable query.
        unsafe { output.cast::<Record>().read() }.validate::<Query>(MEMORY_BUDGETS)?;
        let output = unsafe { &mut *output };
        if output.reserved != 0
            || output.element_size != size_of::<MemoryBudget>() as u32
            || (output.capacity != 0 && output.data.is_null())
        {
            return Err(INVALID);
        }
        if !self.snapshot.memory_budget_supported {
            return Err(UNSUPPORTED);
        }
        let heaps = self.snapshot.memory_heaps.len();
        if output.capacity == 0 || (output.capacity as usize) < heaps {
            output.count = heaps as u32;
            return if output.capacity == 0 {
                Ok(())
            } else {
                Err(CAPACITY)
            };
        }
        // SAFETY: query on the retained live adapter; adopts the reference.
        let adapter3 = self.adapter.cast::<dxgi::IDXGIAdapter3>().map_err(|_| UNSUPPORTED)?;
        let mut values = [MemoryBudget::default(); 16];
        for (i, out) in values[..heaps].iter_mut().enumerate() {
            let group = if self.snapshot.memory_heaps[i].device_local != 0 {
                dxgi::DXGI_MEMORY_SEGMENT_GROUP_LOCAL
            } else {
                dxgi::DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL
            };
            // SAFETY: query writes the out-structure; no object state changes.
            let mut info = dxgi::DXGI_QUERY_VIDEO_MEMORY_INFO::default();
            // SAFETY: query writes the out-structure; no object state changes.
            unsafe { adapter3.QueryVideoMemoryInfo(0, group, &mut info) }.map_err(map_error)?;
            *out = MemoryBudget {
                budget: info.Budget,
                usage: info.CurrentUsage, // Deliberately not clamped to budget.
                heap: i as u32,
                reserved: 0,
            };
        }
        // SAFETY: typed output storage is guaranteed by the caller.
        unsafe { super::copy_out(output, &values[..heaps]) }
    }
}

pub struct Timeline {
    device: *const Device,
    fence: d3d::ID3D12Fence,
}

impl Drop for Timeline {
    fn drop(&mut self) {
        // The fence releases its reference; the caller excluded pending uses.
    }
}

impl Timeline {
    pub(super) fn create(device: &Device, initial: u64) -> Result<Box<Self>, Status> {
        device.ready()?;
        // SAFETY: creation adopts one owned fence reference.
        let fence: d3d::ID3D12Fence = unsafe {
            device
                .device
                .CreateFence(initial, d3d::D3D12_FENCE_FLAG_NONE)
                .map_err(map_error)?
        };
        Ok(Box::new(Self {
            device: ptr::from_ref(device),
            fence,
        }))
    }
    pub(super) fn poll(&self) -> Result<u64, Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        // SAFETY: fence stays live; the call reports the current completed value.
        let value = unsafe { self.fence.GetCompletedValue() };
        Ok(value)
    }
    pub(super) fn wait(&self, value: u64, timeout: u64) -> Result<(), Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        // SAFETY: event creation has no inputs; closed after the wait below.
        let event = unsafe { CreateEventW(None, false, false, None) }.map_err(map_error)?;
        // SAFETY: fence stays live; the event is owned by this call.
        unsafe { self.fence.SetEventOnCompletion(value, HANDLE(event.0)) }.map_err(map_error)?;
        let milliseconds = if timeout == u64::MAX {
            INFINITE
        } else {
            u32::try_from(timeout.div_ceil(1_000_000)).unwrap_or(u32::MAX)
        };
        // SAFETY: handle is a valid event owned by this call.
        let outcome = unsafe { WaitForSingleObject(HANDLE(event.0), milliseconds) };
        // SAFETY: exactly one close of the owned event handle.
        unsafe {
            CloseHandle(HANDLE(event.0)).map_err(map_error)?;
        }
        // WAIT_EVENT(0) is WAIT_OBJECT_0.
        match outcome.0 {
            0 => Ok(()),
            _ if timeout != u64::MAX => Err(TIMEOUT),
            _ => Err(BACKEND_ERROR),
        }
    }
    pub(super) fn signal(&self, value: u64) -> Result<(), Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        // SAFETY: fence stays live; the caller orders monotonic signal values.
        unsafe { self.fence.Signal(value) }.map_err(map_error)?;
        Ok(())
    }
    pub(super) fn fence(&self) -> &d3d::ID3D12Fence {
        &self.fence
    }

    pub(super) fn device(&self) -> *const Device {
        self.device
    }
}

/// Scratch helper: caller-provided storage, exact size/alignment and no
/// hot-path allocation, identical in contract to the Vulkan translation paths.
pub(super) fn scratch(
    pointer: *mut std::ffi::c_void,
    size: u64,
    requirements: HostRequirements,
) -> Result<*mut u8, Status> {
    if size < requirements.size {
        return Err(CAPACITY);
    }
    if requirements.size != 0
        && (pointer.is_null() || pointer as usize % requirements.alignment as usize != 0)
    {
        return Err(INVALID);
    }
    Ok(pointer.cast())
}

pub(super) unsafe fn slice<'a, T>(pointer: *const T, count: u32) -> Result<&'a [T], Status> {
    array_layout::<T>(count)?;
    if count == 0 {
        return Ok(&[]);
    }
    if pointer.is_null() {
        return Err(INVALID);
    }
    // SAFETY: trusted C array extent and alignment; size arithmetic checked above.
    Ok(unsafe { std::slice::from_raw_parts(pointer, count as usize) })
}

pub(super) fn array_layout<T>(count: u32) -> Result<std::alloc::Layout, Status> {
    if count == 0 {
        return Ok(std::alloc::Layout::from_size_align(0, 1).unwrap());
    }
    std::alloc::Layout::array::<T>(count as usize).map_err(|_| INVALID)
}

#[cfg(test)]
mod tests;

/// Create one shader-invisible attachment-view descriptor heap.
unsafe fn create_view_heap(
    device: &d3d::ID3D12Device,
    kind: d3d::D3D12_DESCRIPTOR_HEAP_TYPE,
    count: u32,
) -> Result<d3d::ID3D12DescriptorHeap, Status> {
    let desc = d3d::D3D12_DESCRIPTOR_HEAP_DESC {
        Type: kind,
        NumDescriptors: count,
        Flags: d3d::D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
        NodeMask: 0,
    };
    // SAFETY: well-formed heap description; output reference adopted.
    unsafe {
        device
            .CreateDescriptorHeap::<d3d::ID3D12DescriptorHeap>(&desc)
            .map_err(map_error)
    }
}
