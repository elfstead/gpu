//! Explicit foundation setup. C lifetime/threading contract: include/ogpu_next.h.
//! No execution, allocation, completion-retention or scheduler policy is borrowed
//! from ABI 20. Both paths currently share only the native instance/query loader.
mod commands_api;
mod descriptors_api;
mod executables_api;
mod format_catalog;
mod images_api;
mod memory_api;
mod native;
mod presentation;
mod presentation_api;
pub use commands_api::*;
pub use descriptors_api::*;
pub use executables_api::*;
pub use images_api::*;
pub use memory_api::*;
pub use presentation::Surface;
pub use presentation_api::*;
pub mod rust;
pub mod types;
use crate::vulkan::Instance;
pub use native::Executable;
pub use native::ExecutableCache;
pub use native::Memory;
pub use native::QueryPool;
pub use native::Swapchain;
pub use native::{Arena, List, Split};
pub use native::{Binary, Fence};
pub use native::{Device, Queue, Timeline};
pub use native::{Image, View};
use ogpu_vulkan_sys as vk;
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
    sync::Arc,
};
use types::*;

#[derive(Clone)]
struct Snapshot {
    cache_uuid: [u8; 16],
    info: AdapterInfo,
    queues: Vec<QueueInfo>,
    memory_types: Vec<MemoryTypeInfo>,
    memory_heaps: Vec<MemoryHeapInfo>,
    memory_budget_supported: bool,
    memory_limits: MemoryLimits,
    descriptor_limits: DescriptorLimits,
    execution_limits: ExecutionLimits,
    graphics_limits: GraphicsLimits,
    query_limits: QueryLimits,
    subgroup_limits: SubgroupLimits,
    float_controls: FloatControls,
    matrix_tuples: Vec<MatrixTuple>,
    integer_dot: IntegerDotInfo,
    mesh_limits: MeshLimits,
    formats: Vec<FormatInfo>,
    features: FeatureInfo,
}
pub struct Adapter {
    instance: Arc<Instance>,
    physical: vk::VkPhysicalDevice,
    snapshot: Snapshot,
}
pub struct Discovery {
    adapters: Vec<Adapter>,
}

fn boundary(f: impl FnOnce() -> Result<(), Status>) -> Status {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(())) => OK,
        Ok(Err(status)) => status,
        Err(_) => INTERNAL_ERROR,
    }
}
fn loader_error(error: crate::Error) -> Status {
    if error.status == crate::UNSUPPORTED {
        UNSUPPORTED
    } else {
        BACKEND_ERROR
    }
}

impl Snapshot {
    /// Caller guarantees the complete header and, once validated, query storage.
    unsafe fn query(&self, query: *mut Query) -> Result<(), Status> {
        if query.is_null() {
            return Err(INVALID);
        }
        // SAFETY: every query starts with a readable Record; validate full size
        // before creating a reference to the larger object.
        let header = unsafe { query.cast::<Record>().read() };
        header.validate::<Query>(header.kind)?;
        let query = unsafe { &mut *query };
        if query.reserved != 0 {
            return Err(INVALID);
        }
        // SAFETY: typed output storage is guaranteed by the C caller. copy_out
        // checks schema, capacity and non-null before writing any element.
        unsafe {
            match header.kind {
                INFO => copy_out(query, std::slice::from_ref(&self.info)),
                QUEUES => copy_out(query, &self.queues),
                MEMORY_TYPES => copy_out(query, &self.memory_types),
                MEMORY_HEAPS => copy_out(query, &self.memory_heaps),
                FEATURES => copy_out(query, std::slice::from_ref(&self.features)),
                MEMORY_LIMITS => copy_out(query, std::slice::from_ref(&self.memory_limits)),
                DESCRIPTOR_LIMITS => copy_out(query, std::slice::from_ref(&self.descriptor_limits)),
                EXECUTION_LIMITS => copy_out(query, std::slice::from_ref(&self.execution_limits)),
                GRAPHICS_LIMITS => copy_out(query, std::slice::from_ref(&self.graphics_limits)),
                QUERY_LIMITS => copy_out(query, std::slice::from_ref(&self.query_limits)),
                SUBGROUP_LIMITS => copy_out(query, std::slice::from_ref(&self.subgroup_limits)),
                FLOAT_CONTROLS => copy_out(query, std::slice::from_ref(&self.float_controls)),
                MATRIX_TUPLES => copy_out(query, &self.matrix_tuples),
                INTEGER_DOT_INFO => copy_out(query, std::slice::from_ref(&self.integer_dot)),
                MESH_LIMITS => copy_out(query, std::slice::from_ref(&self.mesh_limits)),
                FORMATS => copy_out(query, &self.formats),
                _ => Err(UNSUPPORTED),
            }
        }
    }
}
unsafe fn copy_out<T: Copy>(query: &mut Query, values: &[T]) -> Result<(), Status> {
    if query.element_size != size_of::<T>() as u32 || (query.capacity != 0 && query.data.is_null())
    {
        return Err(INVALID);
    }
    let count = u32::try_from(values.len()).map_err(|_| INTERNAL_ERROR)?;
    query.count = count;
    if query.capacity == 0 {
        return Ok(());
    }
    if query.capacity < count {
        return Err(CAPACITY);
    }
    // SAFETY: caller owns writable aligned space for capacity elements, distinct
    // from this immutable snapshot and the query itself. T is a C Copy record.
    unsafe {
        ptr::copy_nonoverlapping(values.as_ptr(), query.data.cast::<T>(), values.len());
    }
    Ok(())
}

/// # Safety
/// See discovery_create in include/ogpu_next.h; output is writable/nonoverlapping.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_discovery_create(
    version: u32,
    out: *mut *mut Discovery,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    if version != VERSION {
        return VERSION_MISMATCH;
    }
    boundary(|| {
        let discovery = rust::discover()?;
        unsafe {
            out.write(Box::into_raw(discovery));
        }
        Ok(())
    })
}
/// # Safety
/// Discovery is null or live; see include/ogpu_next.h.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_discovery_count(discovery: *const Discovery) -> u32 {
    unsafe { discovery.as_ref() }.map_or(0, |d| d.adapters.len() as u32)
}
/// # Safety
/// Discovery is null or live. Returned pointer is borrowed until destruction.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_discovery_adapter(
    discovery: *const Discovery,
    index: u32,
) -> *const Adapter {
    unsafe { discovery.as_ref() }
        .and_then(|d| d.adapters.get(index as usize))
        .map_or(ptr::null(), |a| a)
}
/// # Safety
/// See include/ogpu_next.h. Null is allowed; other handles must be uniquely owned.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_discovery_destroy(discovery: *mut Discovery) {
    if !discovery.is_null() {
        unsafe {
            drop(Box::from_raw(discovery));
        }
    }
}
/// # Safety
/// Live adapter and trusted query/output storage; see include/ogpu_next.h.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_adapter_query(
    adapter: *const Adapter,
    query: *mut Query,
) -> Status {
    boundary(|| unsafe { rust::adapter_query(adapter.as_ref().ok_or(INVALID)?, query) })
}
/// # Safety
/// Live adapter, readable description/arrays, writable output; see include/ogpu_next.h.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_device_create(
    adapter: *const Adapter,
    desc: *const DeviceDesc,
    out: *mut *mut Device,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let adapter = unsafe { adapter.as_ref() }.ok_or(INVALID)?;
        if desc.is_null() {
            return Err(INVALID);
        }
        unsafe { desc.cast::<Record>().read() }.validate::<DeviceDesc>(DEVICE_DESC)?;
        let desc = unsafe { &*desc };
        if desc.reserved != 0
            || desc.queue_request_count == 0
            || desc.queues.is_null()
            || desc.queue_request_count as usize > adapter.snapshot.queues.len()
        {
            return Err(INVALID);
        }
        let requests =
            unsafe { std::slice::from_raw_parts(desc.queues, desc.queue_request_count as usize) };
        let device = rust::device_create(adapter, requests, desc.required_features)?;
        unsafe {
            out.write(device.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// Live device and trusted query/output storage; see include/ogpu_next.h.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_device_query(
    device: *const Device,
    query: *mut Query,
) -> Status {
    boundary(|| unsafe {
        rust::device_query(
            rust::DeviceHandle::from_raw(device.cast_mut()).ok_or(INVALID)?,
            query,
        )
    })
}
/// # Safety
/// Live device and trusted query/output storage; explicit live sampling, not a snapshot query.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_memory_budget_query(
    device: *const Device,
    query: *mut Query,
) -> Status {
    boundary(|| unsafe {
        rust::memory_budget_query(
            rust::DeviceHandle::from_raw(device.cast_mut()).ok_or(INVALID)?,
            query,
        )
    })
}
/// # Safety
/// Device is null or live. Returned queue is borrowed; see include/ogpu_next.h.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_device_queue(
    device: *const Device,
    domain: u32,
    index: u32,
) -> *mut Queue {
    unsafe { rust::DeviceHandle::from_raw(device.cast_mut()) }
        .and_then(|d| unsafe { rust::device_queue(d, domain, index) })
        .map_or(ptr::null_mut(), |q| q.as_raw())
}
/// # Safety
/// No children/pending operations may remain. Null is allowed. Never waits.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_device_destroy(device: *mut Device) {
    if let Some(device) = unsafe { rust::DeviceHandle::from_raw(device) } {
        unsafe {
            rust::device_destroy(device);
        }
    }
}
/// # Safety
/// Device outlives the timeline. Output is writable; see include/ogpu_next.h.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_timeline_create(
    device: *mut Device,
    initial: u64,
    out: *mut *mut Timeline,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let device = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let timeline = unsafe { rust::timeline_create(device, initial)? };
        unsafe {
            out.write(timeline.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// No pending operations/access may remain. Null is allowed. Never waits.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_timeline_destroy(timeline: *mut Timeline) {
    if let Some(timeline) = unsafe { rust::TimelineHandle::from_raw(timeline) } {
        unsafe {
            rust::timeline_destroy(timeline);
        }
    }
}
/// # Safety
/// Live timeline and writable nonoverlapping output; see include/ogpu_next.h.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_timeline_poll(
    timeline: *const Timeline,
    out: *mut u64,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    boundary(|| {
        let timeline =
            unsafe { rust::TimelineHandle::from_raw(timeline.cast_mut()) }.ok_or(INVALID)?;
        let completed = unsafe { rust::timeline_poll(timeline)? };
        unsafe {
            out.write(completed);
        }
        Ok(())
    })
}
/// # Safety
/// Point's timeline and device remain live through the wait; see include/ogpu_next.h.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_timeline_wait(point: Point, timeout_ns: u64) -> Status {
    boundary(|| unsafe {
        rust::timeline_wait(
            rust::TimelineHandle::from_raw(point.timeline).ok_or(INVALID)?,
            point.value,
            timeout_ns,
        )
    })
}
/// # Safety
/// Caller orders monotonic values against all host/GPU signals; see include/ogpu_next.h.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_timeline_signal_host(point: Point) -> Status {
    boundary(|| unsafe {
        rust::timeline_signal(
            rust::TimelineHandle::from_raw(point.timeline).ok_or(INVALID)?,
            point.value,
        )
    })
}

#[cfg(test)]
mod tests;
