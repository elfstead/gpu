//! Resource preparation and host descriptor encoding. No automatic ownership.
use super::super::{Executable, ExecutableCache, Image, QueryPool, Split, View};
use super::*;

/// # Safety
/// Live device; readable trusted artifact/metadata arrays and correctly synchronized
/// optional cache. Device outlives executable and all recorded/pending uses.
pub unsafe fn executable_create(
    device: DeviceHandle,
    desc: &ExecutableDesc,
) -> Result<ExecutableHandle> {
    Ok(ExecutableHandle(NonNull::from(Box::leak(unsafe {
        Executable::create(device.get(), desc)?
    }))))
}
/// # Safety
/// Live device, valid initial cache data and declared synchronization mode.
/// Device outlives cache; cache preparation/mutation obeys its exclusion contract.
pub unsafe fn executable_cache_create(
    device: DeviceHandle,
    desc: &ExecutableCacheDesc,
) -> Result<ExecutableCacheHandle> {
    Ok(ExecutableCacheHandle(NonNull::from(Box::leak(unsafe {
        ExecutableCache::create(device.get(), desc)?
    }))))
}
/// Query size with None, or fill caller storage with Some. Size is updated even on
/// CAPACITY as by the native cache export contract; no allocation or retry loop.
/// # Safety
/// Live cache/device, exclusive against mutation/preparation; disjoint output.
pub unsafe fn executable_cache_data(
    cache: ExecutableCacheHandle,
    size: &mut usize,
    data: Option<&mut [u8]>,
) -> Result<()> {
    let pointer = match data {
        None => std::ptr::null_mut(),
        Some(data) => {
            *size = data.len();
            data.as_mut_ptr().cast()
        }
    };
    unsafe { executable_cache_data_raw(cache, size, pointer) }
}
pub(in crate::foundation) unsafe fn executable_cache_data_raw(
    cache: ExecutableCacheHandle,
    size: *mut usize,
    data: *mut std::ffi::c_void,
) -> Result<()> {
    unsafe { cache.get().data(size, data) }
}
/// # Safety
/// Live same-device caches, exclusive destination and no source mutation/destruction.
pub unsafe fn executable_cache_merge(
    destination: ExecutableCacheHandle,
    sources: &[ExecutableCacheHandle],
) -> Result<()> {
    unsafe {
        executable_cache_merge_raw(destination, count(sources.len())?, sources.as_ptr().cast())
    }
}
pub(in crate::foundation) unsafe fn executable_cache_merge_raw(
    destination: ExecutableCacheHandle,
    count: u32,
    sources: *const *mut ExecutableCache,
) -> Result<()> {
    unsafe { destination.get().merge(count, sources) }
}
/// # Safety
/// Live device; valid image profile. No surface/image is implicitly created.
pub unsafe fn image_limits_query(
    device: DeviceHandle,
    profile: &ImageProfile,
) -> Result<ImageLimits> {
    unsafe { device.get() }.image_limits(profile)
}
/// # Safety
/// Live device, valid description arrays and disjoint writable requirements/type storage.
pub unsafe fn image_requirements(
    device: DeviceHandle,
    desc: &ImageDesc,
    out: &mut Requirements,
) -> Result<()> {
    unsafe { device.get().image_requirements(desc, out) }
}
/// # Safety
/// Live device, valid description arrays. Device outlives image and its uses.
pub unsafe fn image_create_unbound(device: DeviceHandle, desc: &ImageDesc) -> Result<ImageHandle> {
    Ok(ImageHandle(NonNull::from(Box::leak(unsafe {
        Image::create(device.get(), desc)?
    }))))
}
/// # Safety
/// Same as unbound creation; placement is valid, compatible and outlives the image.
pub unsafe fn image_create(
    device: DeviceHandle,
    desc: &ImageDesc,
    placement: Span,
) -> Result<ImageHandle> {
    // Retain temporary ownership until bind succeeds: failed binding must not leak
    // or accidentally transfer ownership of the caller's placement allocation.
    let image = unsafe { Image::create(device.get(), desc)? };
    unsafe {
        image.bind(placement)?;
    }
    Ok(ImageHandle(NonNull::from(Box::leak(image))))
}
/// # Safety
/// Unbound live image, exclusive host access; compatible backing outlives all uses.
pub unsafe fn image_bind(image: ImageHandle, placement: Span) -> Result<()> {
    unsafe { image.get().bind(placement) }
}
/// # Safety
/// Live unbound image/device and exclusive binding access. Returned backing and
/// image remain separate owners, requiring explicit destruction in lifetime order.
pub unsafe fn memory_create_dedicated_image(
    image: ImageHandle,
    memory_type: u32,
) -> Result<MemoryHandle> {
    Ok(MemoryHandle(NonNull::from(Box::leak(
        unsafe { image.get() }.dedicated_memory(memory_type)?,
    ))))
}
/// # Safety
/// Live bound image, valid description; image/backing/device outlive view and uses.
pub unsafe fn view_create(image: ImageHandle, desc: &ViewDesc) -> Result<ViewHandle> {
    Ok(ViewHandle(NonNull::from(Box::leak(View::create(
        unsafe { image.get() },
        desc,
    )?))))
}
/// # Safety
/// Live device with declared query features; device outlives pool and its uses.
pub unsafe fn query_pool_create(
    device: DeviceHandle,
    desc: &QueryPoolDesc,
) -> Result<QueryPoolHandle> {
    Ok(QueryPoolHandle(NonNull::from(Box::leak(
        QueryPool::create(unsafe { device.get() }, desc)?,
    ))))
}
/// # Safety
/// Live device; device outlives the split and all recorded/pending uses.
pub unsafe fn split_create(device: DeviceHandle) -> Result<SplitHandle> {
    Ok(SplitHandle(NonNull::from(Box::leak(Split::create(
        unsafe { device.get() },
    )?))))
}

macro_rules! destroy {
    ($name:ident, $handle:ident, $extra:literal) => {
        #[doc = $extra]
        /// # Safety
        /// Object and parent device are live. No recorded, pending or host uses
        /// remain. Destroy exactly once across all handle copies. Never waits.
        pub unsafe fn $name(handle: $handle) {
            unsafe {
                drop(Box::from_raw(handle.as_raw()));
            }
        }
    };
}
destroy!(
    executable_destroy,
    ExecutableHandle,
    "Destroy prepared executable state."
);
destroy!(
    executable_cache_destroy,
    ExecutableCacheHandle,
    "Exclude all cache preparation/merge/export calls."
);
destroy!(
    image_destroy,
    ImageHandle,
    "All image views must already be destroyed; backing remains independently owned."
);
destroy!(
    view_destroy,
    ViewHandle,
    "The parent image and backing must remain live during destruction."
);
destroy!(
    query_pool_destroy,
    QueryPoolHandle,
    "Retire all query recording, execution and result copies."
);
destroy!(
    split_destroy,
    SplitHandle,
    "Retire every recorded release/acquire/reset referencing the split."
);

pub fn descriptor_scratch_requirements(kind: u32, count: u32) -> Result<HostRequirements> {
    native::descriptor_scratch(kind, count)
}
/// # Safety
/// Live device/resources; output byte spans and scratch are writable, properly
/// aligned and disjoint from inputs/each other. Descriptor contents own nothing;
/// caller obeys descriptor mutation and resource-lifetime contracts.
pub unsafe fn write_resource_descriptors(
    device: DeviceHandle,
    inputs: &[ResourceDescriptor],
    outputs: &[HostSpan],
    scratch: HostSpan,
) -> Result<()> {
    if inputs.len() != outputs.len() {
        return Err(INVALID);
    }
    unsafe {
        write_resource_descriptors_raw(
            device,
            count(inputs.len())?,
            inputs.as_ptr(),
            outputs.as_ptr(),
            scratch,
        )
    }
}
/// # Safety
/// Live device; sampler descriptions/extension storage valid; output byte spans
/// and scratch writable, aligned and disjoint, with caller-ordered heap mutation.
pub unsafe fn write_sampler_descriptors(
    device: DeviceHandle,
    inputs: &[SamplerDesc],
    outputs: &[HostSpan],
    scratch: HostSpan,
) -> Result<()> {
    if inputs.len() != outputs.len() {
        return Err(INVALID);
    }
    unsafe {
        write_sampler_descriptors_raw(
            device,
            count(inputs.len())?,
            inputs.as_ptr(),
            outputs.as_ptr(),
            scratch,
        )
    }
}
pub(in crate::foundation) unsafe fn write_resource_descriptors_raw(
    device: DeviceHandle,
    count: u32,
    inputs: *const ResourceDescriptor,
    outputs: *const HostSpan,
    scratch: HostSpan,
) -> Result<()> {
    unsafe {
        device
            .get()
            .write_resources(count, inputs, outputs, scratch)
    }
}
pub(in crate::foundation) unsafe fn write_sampler_descriptors_raw(
    device: DeviceHandle,
    count: u32,
    inputs: *const SamplerDesc,
    outputs: *const HostSpan,
    scratch: HostSpan,
) -> Result<()> {
    unsafe { device.get().write_samplers(count, inputs, outputs, scratch) }
}
