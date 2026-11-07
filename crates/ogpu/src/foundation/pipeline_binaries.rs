//! Explicit native pipeline binaries. Cold preparation only; no persistence or fallback.
use super::*;
use std::{cell::Cell, ffi::c_void};
pub(super) struct Commands {
    create: vk::PFN_vkCreatePipelineBinariesKHR,
    destroy: vk::PFN_vkDestroyPipelineBinaryKHR,
    data: vk::PFN_vkGetPipelineBinaryDataKHR,
    key: vk::PFN_vkGetPipelineKeyKHR,
    release: vk::PFN_vkReleaseCapturedPipelineDataKHR,
}
impl Commands {
    pub fn load(instance: &Instance, enabled: FeatureBits) -> Result<Self, Status> {
        macro_rules! get {
            ($name:literal,$ty:ty) => {{
                if enabled & PIPELINE_BINARIES == 0 {
                    None
                } else {
                    let f = unsafe {
                        std::mem::transmute::<vk::PFN_vkVoidFunction, $ty>(instance.proc($name))
                    };
                    if f.is_none() {
                        return Err(UNSUPPORTED);
                    }
                    f
                }
            }};
        }
        Ok(Self {
            create: get!(
                c"vkCreatePipelineBinariesKHR",
                vk::PFN_vkCreatePipelineBinariesKHR
            ),
            destroy: get!(
                c"vkDestroyPipelineBinaryKHR",
                vk::PFN_vkDestroyPipelineBinaryKHR
            ),
            data: get!(
                c"vkGetPipelineBinaryDataKHR",
                vk::PFN_vkGetPipelineBinaryDataKHR
            ),
            key: get!(c"vkGetPipelineKeyKHR", vk::PFN_vkGetPipelineKeyKHR),
            release: get!(
                c"vkReleaseCapturedPipelineDataKHR",
                vk::PFN_vkReleaseCapturedPipelineDataKHR
            ),
        })
    }
}
pub(super) fn feature(bits: FeatureBits) -> vk::VkPhysicalDevicePipelineBinaryFeaturesKHR {
    vk::VkPhysicalDevicePipelineBinaryFeaturesKHR {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PIPELINE_BINARY_FEATURES_KHR,
        pipelineBinaries: u32::from(bits & PIPELINE_BINARIES != 0),
        ..Default::default()
    }
}
pub(super) fn policy(bits: FeatureBits) -> vk::VkDevicePipelineBinaryInternalCacheControlKHR {
    vk::VkDevicePipelineBinaryInternalCacheControlKHR {
        sType:
            vk::VkStructureType_VK_STRUCTURE_TYPE_DEVICE_PIPELINE_BINARY_INTERNAL_CACHE_CONTROL_KHR,
        disableInternalCache: u32::from(bits & DISABLE_BINARY_INTERNAL_CACHE != 0),
        ..Default::default()
    }
}
pub(super) fn snapshot(
    instance: &Instance,
    physical: vk::VkPhysicalDevice,
    f: &Functions,
) -> Result<(FeatureBits, PipelineBinaryLimits), Status> {
    if !instance
        .supports_extension(physical, c"VK_KHR_pipeline_binary")
        .map_err(loader_error)?
    {
        return Ok((FeatureBits::ZERO, PipelineBinaryLimits::default()));
    }
    let mut binary = feature(FeatureBits::ZERO);
    let mut root = vk::VkPhysicalDeviceFeatures2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2,
        pNext: ptr::from_mut(&mut binary).cast(),
        ..Default::default()
    };
    unsafe {
        (f.vkGetPhysicalDeviceFeatures2.unwrap())(physical, &mut root);
    }
    let mut p = vk::VkPhysicalDevicePipelineBinaryPropertiesKHR {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PIPELINE_BINARY_PROPERTIES_KHR,
        ..Default::default()
    };
    let mut root = vk::VkPhysicalDeviceProperties2 {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2,
        pNext: ptr::from_mut(&mut p).cast(),
        ..Default::default()
    };
    unsafe {
        (f.vkGetPhysicalDeviceProperties2.unwrap())(physical, &mut root);
    }
    let bits = if binary.pipelineBinaries != 0 {
        PIPELINE_BINARIES
            | if p.pipelineBinaryInternalCacheControl != 0 {
                DISABLE_BINARY_INTERNAL_CACHE
            } else {
                FeatureBits::ZERO
            }
    } else {
        FeatureBits::ZERO
    };
    Ok((
        bits,
        PipelineBinaryLimits {
            internal_cache: p.pipelineBinaryInternalCache,
            internal_cache_control: p.pipelineBinaryInternalCacheControl,
            prefers_internal_cache: p.pipelineBinaryPrefersInternalCache,
            precompiled_internal_cache: p.pipelineBinaryPrecompiledInternalCache,
            compressed_data: p.pipelineBinaryCompressedData,
            reserved: 0,
        },
    ))
}
fn native_key(key: &ArtifactKey) -> Result<vk::VkPipelineBinaryKeyKHR, Status> {
    if key.size == 0 || key.size > 32 {
        return Err(INVALID);
    }
    Ok(vk::VkPipelineBinaryKeyKHR {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_BINARY_KEY_KHR,
        keySize: key.size,
        key: key.data,
        ..Default::default()
    })
}
fn public_key(key: &vk::VkPipelineBinaryKeyKHR) -> Result<ArtifactKey, Status> {
    if key.keySize == 0 || key.keySize > 32 {
        return Err(BACKEND_ERROR);
    }
    let mut out = ArtifactKey {
        size: key.keySize,
        ..Default::default()
    };
    out.data[..key.keySize as usize].copy_from_slice(&key.key[..key.keySize as usize]);
    Ok(out)
}
fn same_key(a: &ArtifactKey, b: &ArtifactKey) -> Result<bool, Status> {
    native_key(a)?;
    native_key(b)?;
    Ok(a.size == b.size && a.data[..a.size as usize] == b.data[..b.size as usize])
}
pub(in crate::foundation) fn identity(d: &Device) -> Result<ArtifactKey, Status> {
    key(d, ptr::null())
}
fn key(d: &Device, info: *const vk::VkPipelineCreateInfoKHR) -> Result<ArtifactKey, Status> {
    d.ready()?;
    let mut out = vk::VkPipelineBinaryKeyKHR {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_BINARY_KEY_KHR,
        ..Default::default()
    };
    unsafe {
        d.result((d.binary_commands.key.ok_or(UNSUPPORTED)?)(
            d.handle, info, &mut out,
        ))?;
    }
    public_key(&out)
}
pub struct PipelineBinary {
    device: *const Device,
    handle: vk::VkPipelineBinaryKHR,
}
impl Drop for PipelineBinary {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            let d = unsafe { &*self.device };
            unsafe {
                (d.binary_commands.destroy.unwrap())(d.handle, self.handle, ptr::null());
            }
        }
    }
}
impl PipelineBinary {
    pub(in crate::foundation) unsafe fn import(
        d: &Device,
        global: &ArtifactKey,
        binary_key: &ArtifactKey,
        data: Bytes,
    ) -> Result<Box<Self>, Status> {
        d.ready()?;
        if !same_key(global, &identity(d)?)? {
            return Err(UNSUPPORTED);
        }
        let k = native_key(binary_key)?;
        if data.size == 0 || data.size > isize::MAX as usize || data.data.is_null() {
            return Err(INVALID);
        }
        let bytes = vk::VkPipelineBinaryDataKHR {
            dataSize: data.size,
            pData: data.data.cast_mut(),
        };
        let keys = vk::VkPipelineBinaryKeysAndDataKHR {
            binaryCount: 1,
            pPipelineBinaryKeys: &k,
            pPipelineBinaryData: &bytes,
        };
        let info = vk::VkPipelineBinaryCreateInfoKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_BINARY_CREATE_INFO_KHR,
            pKeysAndDataInfo: &keys,
            ..Default::default()
        };
        let mut out = ptr::null_mut();
        let mut count = 0;
        unsafe {
            create(d, &info, 1, &mut out, &mut count)?;
        }
        if count != 1 || out.is_null() {
            return Err(BACKEND_ERROR);
        }
        Ok(unsafe { Box::from_raw(out) })
    }
    pub(in crate::foundation) unsafe fn export(
        &self,
        out: &mut ArtifactKey,
        size: &mut usize,
        data: *mut c_void,
    ) -> Result<(), Status> {
        let d = unsafe { &*self.device };
        d.ready()?;
        if !data.is_null() && *size > isize::MAX as usize {
            return Err(INVALID);
        }
        let info = vk::VkPipelineBinaryDataInfoKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_BINARY_DATA_INFO_KHR,
            pipelineBinary: self.handle,
            ..Default::default()
        };
        let mut key = vk::VkPipelineBinaryKeyKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_BINARY_KEY_KHR,
            ..Default::default()
        };
        let result = unsafe {
            (d.binary_commands.data.ok_or(UNSUPPORTED)?)(d.handle, &info, &mut key, size, data)
        };
        if result == vk::VkResult_VK_ERROR_NOT_ENOUGH_SPACE_KHR {
            return Err(CAPACITY);
        }
        d.result(result)?;
        *out = public_key(&key)?;
        Ok(())
    }
}
// Native partial results are owned immediately, even when a later binary fails.
struct Handles<'a> {
    d: &'a Device,
    values: Vec<vk::VkPipelineBinaryKHR>,
}
impl Drop for Handles<'_> {
    fn drop(&mut self) {
        for h in &self.values {
            if !h.is_null() {
                unsafe {
                    (self.d.binary_commands.destroy.unwrap())(self.d.handle, *h, ptr::null());
                }
            }
        }
    }
}
unsafe fn create(
    d: &Device,
    info: &vk::VkPipelineBinaryCreateInfoKHR,
    capacity: u32,
    out: *mut *mut PipelineBinary,
    count: &mut u32,
) -> Result<(), Status> {
    d.ready()?;
    let call = d.binary_commands.create.ok_or(UNSUPPORTED)?;
    if capacity != 0 && out.is_null() {
        return Err(INVALID);
    }
    if out.is_null() && capacity == 0 {
        let mut native = vk::VkPipelineBinaryHandlesInfoKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_BINARY_HANDLES_INFO_KHR,
            ..Default::default()
        };
        let result = unsafe { call(d.handle, info, ptr::null(), &mut native) };
        *count = native.pipelineBinaryCount;
        return binary_status(d, result);
    }
    if capacity == 0 {
        return Err(INVALID);
    }
    if capacity as u64 > isize::MAX as u64 / size_of::<*mut PipelineBinary>() as u64 {
        return Err(INVALID);
    }
    let outputs = unsafe { std::slice::from_raw_parts_mut(out, capacity as usize) };
    outputs.fill(ptr::null_mut());
    let mut handles = Handles {
        d,
        values: Vec::new(),
    };
    handles
        .values
        .try_reserve_exact(capacity as usize)
        .map_err(|_| OUT_OF_MEMORY)?;
    handles.values.resize(capacity as usize, ptr::null_mut());
    let mut native = vk::VkPipelineBinaryHandlesInfoKHR {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_BINARY_HANDLES_INFO_KHR,
        pipelineBinaryCount: capacity,
        pPipelineBinaries: handles.values.as_mut_ptr(),
        ..Default::default()
    };
    let result = unsafe { call(d.handle, info, ptr::null(), &mut native) };
    *count = native.pipelineBinaryCount;
    binary_status(d, result)?;
    if native.pipelineBinaryCount > capacity
        || handles.values[..native.pipelineBinaryCount as usize]
            .iter()
            .any(|h| h.is_null())
    {
        return Err(BACKEND_ERROR);
    }
    // Allocate every wrapper before exposing ownership. A panic leaves guards live.
    let mut wrappers = Vec::new();
    wrappers
        .try_reserve_exact(native.pipelineBinaryCount as usize)
        .map_err(|_| OUT_OF_MEMORY)?;
    for handle in &mut handles.values[..native.pipelineBinaryCount as usize] {
        let binary = Box::new(PipelineBinary {
            device: d,
            handle: *handle,
        });
        *handle = ptr::null_mut();
        wrappers.push(binary);
    }
    for (slot, binary) in outputs.iter_mut().zip(wrappers) {
        *slot = Box::into_raw(binary);
    }
    Ok(())
}
fn binary_status(d: &Device, result: vk::VkResult) -> Result<(), Status> {
    match result {
        vk::VkResult_VK_PIPELINE_BINARY_MISSING_KHR => Err(BINARY_MISSING),
        vk::VkResult_VK_INCOMPLETE => Err(CAPACITY),
        _ => d.result(result),
    }
}
pub(in crate::foundation) unsafe fn capture(
    e: &Executable,
    capacity: u32,
    out: *mut *mut PipelineBinary,
    count: &mut u32,
) -> Result<(), Status> {
    *count = 0;
    unsafe {
        clear_outputs(capacity, out)?;
    }
    if !e.capture_data.get() {
        return Err(INVALID);
    }
    let d = unsafe { &*e.device };
    let info = vk::VkPipelineBinaryCreateInfoKHR {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_BINARY_CREATE_INFO_KHR,
        pipeline: e.pipeline,
        ..Default::default()
    };
    unsafe { create(d, &info, capacity, out, count) }
}
pub(in crate::foundation) fn release(e: &Executable) -> Result<(), Status> {
    let d = unsafe { &*e.device };
    d.ready()?;
    if !e.capture_data.get() {
        return Err(INVALID);
    }
    let info = vk::VkReleaseCapturedPipelineDataInfoKHR {
        sType: vk::VkStructureType_VK_STRUCTURE_TYPE_RELEASE_CAPTURED_PIPELINE_DATA_INFO_KHR,
        pipeline: e.pipeline,
        ..Default::default()
    };
    unsafe {
        d.result((d.binary_commands.release.ok_or(UNSUPPORTED)?)(
            d.handle,
            &info,
            ptr::null(),
        ))?;
    }
    e.capture_data.set(false);
    Ok(())
}
pub(super) struct Input {
    handles: Vec<vk::VkPipelineBinaryKHR>,
}
impl Input {
    pub unsafe fn new(d: &Device, desc: &ExecutableDesc) -> Result<Self, Status> {
        unsafe {
            local_read::parent::<ExecutableDesc>(&desc.header, EXECUTABLE_DESC)?;
        }
        let mut handles = Vec::new();
        if !desc.header.next.is_null() {
            if d.snapshot.features.enabled & PIPELINE_BINARIES == 0 {
                return Err(UNSUPPORTED);
            }
            let p = unsafe {
                executables::record::<ExecutableBinaries>(desc.header.next, EXECUTABLE_BINARIES)?
            };
            if p.count == 0
                || p.reserved != 0
                || !desc.cache.is_null()
                || desc.compile_flags & 1 != 0
            {
                return Err(INVALID);
            }
            let binaries = unsafe { commands::slice(p.binaries, p.count)? };
            handles
                .try_reserve_exact(binaries.len())
                .map_err(|_| OUT_OF_MEMORY)?;
            for b in binaries {
                let b = unsafe { b.as_ref() }.ok_or(INVALID)?;
                if !ptr::eq(b.device, d) {
                    return Err(INVALID);
                }
                handles.push(b.handle);
            }
        }
        if desc.compile_flags & COMPILE_CAPTURE_BINARY != 0 && !desc.cache.is_null() {
            return Err(INVALID);
        }
        Ok(Self { handles })
    }
    pub fn used(&self) -> bool {
        !self.handles.is_empty()
    }
    pub fn native(&self, next: *const c_void) -> vk::VkPipelineBinaryInfoKHR {
        vk::VkPipelineBinaryInfoKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_BINARY_INFO_KHR,
            pNext: next,
            binaryCount: self.handles.len() as u32,
            pPipelineBinaries: self.handles.as_ptr(),
        }
    }
    pub fn link(&self, info: &vk::VkPipelineBinaryInfoKHR, next: *const c_void) -> *const c_void {
        if self.used() {
            ptr::from_ref(info).cast()
        } else {
            next
        }
    }
}
pub(in crate::foundation) enum Action<'a> {
    Create,
    Key(&'a mut ArtifactKey),
    Lookup {
        capacity: u32,
        out: *mut *mut PipelineBinary,
        count: &'a mut u32,
    },
}
impl Action<'_> {
    pub fn source_only(&self) -> bool {
        !matches!(self, Self::Create)
    }
    pub unsafe fn resolve(&mut self, d: &Device, info: *const c_void) -> Result<bool, Status> {
        let root = vk::VkPipelineCreateInfoKHR {
            sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_CREATE_INFO_KHR,
            pNext: info.cast_mut(),
        };
        match self {
            Self::Create => Ok(false),
            Self::Key(out) => {
                **out = key(d, &root)?;
                Ok(true)
            }
            Self::Lookup {
                capacity,
                out,
                count,
            } => {
                if d.snapshot.pipeline_binary_limits.internal_cache == 0
                    || d.snapshot.features.enabled & DISABLE_BINARY_INTERNAL_CACHE != 0
                {
                    return Err(UNSUPPORTED);
                }
                let info = vk::VkPipelineBinaryCreateInfoKHR {
                    sType: vk::VkStructureType_VK_STRUCTURE_TYPE_PIPELINE_BINARY_CREATE_INFO_KHR,
                    pPipelineCreateInfo: &root,
                    ..Default::default()
                };
                unsafe {
                    create(d, &info, *capacity, *out, count)?;
                }
                Ok(true)
            }
        }
    }
}
pub(super) fn capture_state(flags: u32) -> Cell<bool> {
    Cell::new(flags & COMPILE_CAPTURE_BINARY != 0)
}

pub(super) unsafe fn shader_code(shader: &Shader, binary: bool) -> Result<(), Status> {
    if shader.entry.is_null() {
        return Err(INVALID);
    }
    if binary {
        return Ok(());
    }
    if shader.code.data.is_null()
        || shader.code.data as usize % 4 != 0
        || shader.code.size < 20
        || shader.code.size % 4 != 0
        || shader.code.size > isize::MAX as usize
    {
        return Err(INVALID);
    }
    if unsafe { shader.code.data.cast::<u32>().read() } != 0x07230203 {
        return Err(INVALID);
    }
    Ok(())
}
pub(in crate::foundation) unsafe fn clear_outputs(
    capacity: u32,
    out: *mut *mut PipelineBinary,
) -> Result<(), Status> {
    if capacity == 0 {
        return if out.is_null() { Ok(()) } else { Err(INVALID) };
    }
    if out.is_null()
        || capacity as u64 > isize::MAX as u64 / size_of::<*mut PipelineBinary>() as u64
    {
        return Err(INVALID);
    }
    unsafe { std::slice::from_raw_parts_mut(out, capacity as usize) }.fill(ptr::null_mut());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_uses_only_defined_key_bytes_and_rejects_invalid_sizes() {
        let a = ArtifactKey {
            size: 4,
            data: [7; 32],
        };
        let mut b = a;
        b.data[4..].fill(99);
        assert_eq!(same_key(&a, &b), Ok(true));
        b.data[0] ^= 1;
        assert_eq!(same_key(&a, &b), Ok(false));
        assert!(native_key(&ArtifactKey::default()).is_err());
        assert!(native_key(&ArtifactKey { size: 33, ..a }).is_err());
        let normalized = public_key(&native_key(&a).unwrap()).unwrap();
        assert_eq!(&normalized.data[4..], &[0; 28]);
        assert_eq!(
            validate_feature_dependencies(DISABLE_BINARY_INTERNAL_CACHE),
            Err(INVALID)
        );
        assert_eq!(
            validate_feature_dependencies(DISABLE_BINARY_INTERNAL_CACHE | PIPELINE_BINARIES),
            Ok(())
        );
        assert_eq!(policy(PIPELINE_BINARIES).disableInternalCache, 0);
        assert_eq!(
            policy(PIPELINE_BINARIES | DISABLE_BINARY_INTERNAL_CACHE).disableInternalCache,
            1
        );
    }
    #[test]
    fn output_initialization_and_ignored_source_follow_explicit_binary_input() {
        let mut out = [ptr::dangling_mut::<PipelineBinary>(); 3];
        unsafe {
            clear_outputs(3, out.as_mut_ptr()).unwrap();
        }
        assert!(out.iter().all(|p| p.is_null()));
        assert_eq!(unsafe { clear_outputs(1, ptr::null_mut()) }, Err(INVALID));
        let shader = Shader {
            stage: 4,
            format: 0,
            code: Bytes {
                data: ptr::null(),
                size: 0,
            },
            entry: c"main".as_ptr(),
            specialization: ptr::null(),
            subgroup: ptr::null(),
        };
        assert_eq!(unsafe { shader_code(&shader, false) }, Err(INVALID));
        assert_eq!(unsafe { shader_code(&shader, true) }, Ok(()));
    }

    thread_local! { static MODE:Cell<u32>=const {Cell::new(0)}; static DESTROYS:Cell<u32>=const {Cell::new(0)}; static CREATES:Cell<u32>=const {Cell::new(0)}; }
    unsafe extern "C" fn fake_create(
        _: vk::VkDevice,
        _: *const vk::VkPipelineBinaryCreateInfoKHR,
        _: *const vk::VkAllocationCallbacks,
        out: *mut vk::VkPipelineBinaryHandlesInfoKHR,
    ) -> vk::VkResult {
        CREATES.set(CREATES.get() + 1);
        let out = unsafe { &mut *out };
        let mode = MODE.get();
        if mode == 0 {
            out.pipelineBinaryCount = 0;
            return vk::VkResult_VK_PIPELINE_BINARY_MISSING_KHR;
        }
        assert_eq!(out.pipelineBinaryCount, 2);
        unsafe {
            *out.pPipelineBinaries = ptr::dangling_mut();
            if mode != 1 {
                *out.pPipelineBinaries.add(1) = ptr::dangling_mut();
            }
        }
        out.pipelineBinaryCount = if mode == 3 { 3 } else { 2 };
        match mode {
            1 => vk::VkResult_VK_ERROR_OUT_OF_HOST_MEMORY,
            2 => vk::VkResult_VK_INCOMPLETE,
            _ => vk::VkResult_VK_SUCCESS,
        }
    }
    unsafe extern "C" fn fake_destroy(
        _: vk::VkDevice,
        _: vk::VkPipelineBinaryKHR,
        _: *const vk::VkAllocationCallbacks,
    ) {
        DESTROYS.set(DESTROYS.get() + 1);
    }
    #[test]
    #[ignore = "requires Vulkan device setup; injected partial binary creation checks ownership cleanup"]
    fn gpu_binary_partial_failure_cleanup() {
        let instance = Arc::new(Instance::new().unwrap());
        let physical = instance
            .physical_devices()
            .unwrap()
            .into_iter()
            .find(|p| {
                super::super::snapshot(&instance, *p).is_ok_and(|s| {
                    s.features.baseline_supported != 0
                        && s.features.available & PIPELINE_BINARIES != 0
                })
            })
            .expect("pipeline binaries unavailable");
        let snapshot = super::super::snapshot(&instance, physical).unwrap();
        let domain = snapshot
            .queues
            .iter()
            .find(|q| q.count > 0 && q.flags & COMPUTE != 0)
            .unwrap()
            .domain;
        let adapter = Adapter {
            instance,
            physical,
            snapshot,
        };
        let mut d = Device::create(
            &adapter,
            &[QueueRequest {
                domain,
                count: 1,
                priority: 1.0,
            }],
            PIPELINE_BINARIES,
        )
        .unwrap();
        d.binary_commands.create = Some(fake_create);
        d.binary_commands.destroy = Some(fake_destroy);
        let info = vk::VkPipelineBinaryCreateInfoKHR::default();
        for mode in 0..=4 {
            MODE.set(mode);
            CREATES.set(0);
            DESTROYS.set(0);
            let mut out = [ptr::dangling_mut::<PipelineBinary>(); 2];
            let mut count = 0;
            let result = unsafe { create(&d, &info, 2, out.as_mut_ptr(), &mut count) };
            assert_eq!(CREATES.get(), 1); // No cache-miss recompilation or retry.
            if mode < 4 {
                assert_eq!(
                    result,
                    Err([BINARY_MISSING, OUT_OF_MEMORY, CAPACITY, BACKEND_ERROR][mode as usize])
                );
                assert!(out.iter().all(|p| p.is_null()));
                assert_eq!(DESTROYS.get(), [0, 1, 2, 2][mode as usize]);
            } else {
                assert_eq!(result, Ok(()));
                assert_eq!(DESTROYS.get(), 0);
                for p in out {
                    drop(unsafe { Box::from_raw(p) });
                }
                assert_eq!(DESTROYS.get(), 2);
            }
        }
    }
    #[test]
    #[ignore = "requires Vulkan; verifies binary features and independent internal-cache policy"]
    fn gpu_pipeline_binary_profiles() {
        let instance = Arc::new(Instance::new().unwrap());
        let physical = instance
            .physical_devices()
            .unwrap()
            .into_iter()
            .find(|p| {
                super::super::snapshot(&instance, *p)
                    .is_ok_and(|s| s.features.baseline_supported != 0)
            })
            .unwrap();
        let snapshot = super::super::snapshot(&instance, physical).unwrap();
        let domain = snapshot
            .queues
            .iter()
            .find(|q| q.count > 0 && q.flags & COMPUTE != 0)
            .unwrap()
            .domain;
        println!(
            "PIPELINE_BINARY_LIMITS={:?}",
            snapshot.pipeline_binary_limits
        );
        let adapter = Adapter {
            instance,
            physical,
            snapshot,
        };
        for bits in [
            PIPELINE_BINARIES,
            PIPELINE_BINARIES | DISABLE_BINARY_INTERNAL_CACHE,
        ] {
            let result = Device::create(
                &adapter,
                &[QueueRequest {
                    domain,
                    count: 1,
                    priority: 1.0,
                }],
                bits,
            );
            if bits & !adapter.snapshot.features.available != 0 {
                assert!(matches!(result, Err(UNSUPPORTED)));
                println!("BINARY_FEATURES_UNAVAILABLE={:?}", features::decode(bits));
            } else {
                let d = result.unwrap();
                println!(
                    "BINARY_FEATURES_ENABLED={:?} identity={:?}",
                    features::decode(bits),
                    identity(&d).unwrap()
                );
            }
        }
    }
}
