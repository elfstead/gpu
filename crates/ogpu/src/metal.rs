//! Native Metal implementation of the backend-independent compute slice.
#![allow(clippy::missing_safety_doc)]
#![allow(unexpected_cfgs)] // objc 0.2 selectors probe a historical cargo-clippy feature.
use crate::contract;
mod copy;
mod feedback;
#[cfg(test)]
#[path = "metal/tests.rs"]
mod native_tests;
#[cfg(feature = "spirv-to-msl")]
mod spirv;
use crate::{
    Error, OgpuCapabilities, OgpuDeviceInfo, OgpuDeviceLimits, OgpuError, OgpuProbe, OgpuResult,
    OgpuShaderDesc, OgpuTimingInfo, INTERNAL_ERROR, INVALID_ARGUMENT, OUT_OF_RANGE, UNSUPPORTED,
};
use ::metal::{
    Buffer as MetalBuffer, ComputePipelineState, Device as MetalDevice, FunctionConstantValues,
    MTLResourceOptions, MTLSize,
};
use foreign_types::ForeignType;
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::{
    cell::{OnceCell, RefCell},
    collections::BTreeMap,
    ffi::c_void,
    ptr,
    rc::{Rc, Weak},
    sync::Arc,
};

/// Retained Objective-C object used for Metal 4 types not yet exposed by metal-rs.
struct Mtl4(*mut Object);

impl Mtl4 {
    unsafe fn owned(pointer: *mut Object, what: &str) -> Result<Self, Error> {
        if pointer.is_null() {
            Err(fail(
                INTERNAL_ERROR,
                format!("Metal 4 {what} creation failed"),
            ))
        } else {
            Ok(Self(pointer))
        }
    }

    unsafe fn retained(pointer: *mut Object, what: &str) -> Result<Self, Error> {
        let object = unsafe { Self::owned(pointer, what)? };
        unsafe {
            let _: () = msg_send![object.0, retain];
        }
        Ok(object)
    }
}

impl Drop for Mtl4 {
    fn drop(&mut self) {
        unsafe {
            let _: () = msg_send![self.0, release];
        }
    }
}

pub struct OgpuDevice {
    inner: Rc<Device>,
}
pub struct OgpuBuffer {
    inner: Rc<Buffer>,
}
pub struct OgpuKernel {
    inner: Rc<Kernel>,
}
pub struct OgpuBatch {
    inner: Batch,
}
pub struct OgpuCompletion {
    inner: Completion,
}
pub enum OgpuImage {}
pub enum OgpuImageHeap {}
pub enum OgpuSamplerHeap {}
pub enum OgpuRaster {}

struct Device {
    raw: MetalDevice,
    queue: Mtl4,
    residency: Mtl4,
    buffers: RefCell<BTreeMap<usize, Weak<Buffer>>>,
    byte_copy: OnceCell<ComputePipelineState>,
}

fn make_device(raw: MetalDevice) -> Result<Rc<Device>, Error> {
    unsafe {
        let queue = Mtl4::owned(
            msg_send![raw.as_ptr(), newMTL4CommandQueue],
            "command queue",
        )?;
        let descriptor = Mtl4::owned(
            msg_send![class!(MTLResidencySetDescriptor), new],
            "residency descriptor",
        )?;
        let residency = Mtl4::owned(
            msg_send![raw.as_ptr(), newResidencySetWithDescriptor: descriptor.0 error: ptr::null_mut::<*mut Object>()],
            "residency set",
        )?;
        let _: () = msg_send![queue.0, addResidencySet: residency.0];
        Ok(Rc::new(Device {
            raw,
            queue,
            residency,
            buffers: RefCell::new(BTreeMap::new()),
            byte_copy: OnceCell::new(),
        }))
    }
}

struct Buffer {
    device: Rc<Device>,
    raw: MetalBuffer,
    size: usize,
    host: bool,
}

impl Drop for Buffer {
    fn drop(&mut self) {
        // Remove dead allocations immediately; dispatch cost follows live buffers,
        // not the number ever allocated. This registry does not retain resources.
        self.device
            .buffers
            .borrow_mut()
            .remove(&(self as *const Self as usize));
        unsafe {
            let _: () = msg_send![self.device.residency.0, removeAllocation: self.raw.as_ptr()];
            let _: () = msg_send![self.device.residency.0, commit];
        }
    }
}

struct Kernel {
    device: Rc<Device>,
    pipeline: ComputePipelineState,
    group: MTLSize,
    push_size: u32,
}

struct Batch {
    device: Rc<Device>,
    recording: Option<Recording>,
    retained: Vec<Rc<Buffer>>,
    transient: Vec<MetalBuffer>,
    tables: Vec<Mtl4>,
}

// Release the encoder/command buffer before their backing allocator. A consumed
// batch owns none of these objects, even when its public handle remains alive.
struct Recording {
    encoder: Option<Mtl4>,
    commands: Mtl4,
    _allocator: Mtl4,
}

impl Recording {
    fn end(&mut self) {
        if let Some(encoder) = self.encoder.take() {
            unsafe {
                let _: () = msg_send![encoder.0, endEncoding];
                let _: () = msg_send![self.commands.0, endCommandBuffer];
            }
        }
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        self.end();
    }
}

struct Completion {
    _device: Rc<Device>,
    feedback: Arc<feedback::Feedback>,
    submission: contract::Submission<SubmissionResources, Result<(), Error>>,
}

// Field order matters: release the native command buffer before OGPU allocations.
struct SubmissionResources {
    recording: Recording,
    _transient: Vec<MetalBuffer>,
    _tables: Vec<Mtl4>,
    _retained: Vec<Rc<Buffer>>,
}

pub(crate) fn device_info(device: &MetalDevice) -> OgpuDeviceInfo {
    let mut name = [0; 256];
    for (to, from) in name
        .iter_mut()
        .zip(device.name().as_bytes().iter().take(255))
    {
        *to = *from as _;
    }
    OgpuDeviceInfo {
        name,
        backend: crate::BACKEND_METAL,
        vendor_id: if device.supports_family(::metal::MTLGPUFamily::Apple7) {
            0x106b
        } else {
            0
        },
        device_id: device.registry_id() as u32,
        device_type: 1,
        vulkan_api_major: 0,
        vulkan_api_minor: 0,
        vulkan_api_patch: 0,
        capabilities: if supported(device) {
            capabilities()
        } else {
            OgpuCapabilities::default()
        },
    }
}

fn capabilities() -> OgpuCapabilities {
    OgpuCapabilities {
        compute_queue: 1,
        buffer_device_address: 1,
        storage_buffer_8bit_access: 1,
        storage_buffer_16bit_access: 1,
        shader_float16: 1,
        shader_int8: 1,
        shader_int16: 1,
        shader_int64: 1,
        ..Default::default()
    }
}

fn fail(status: OgpuResult, message: impl Into<String>) -> Error {
    Error::new(status, message)
}

use crate::boundary::{call, create, required};

fn limits(d: &MetalDevice) -> OgpuDeviceLimits {
    let group = d.max_threads_per_threadgroup();
    OgpuDeviceLimits {
        max_group_size: [group.width as u32, group.height as u32, group.depth as u32],
        max_group_invocations: group.width as u32,
        max_shared_memory_bytes: d.max_threadgroup_memory_length() as u32,
        max_dispatch: [u32::MAX; 3],
        max_image_1d: 0,
        max_image_2d: 0,
        max_push_data_bytes: 4096,
    }
}

use crate::contract::range as check_range;

#[cfg(test)]
thread_local! {
    static FAIL_NEXT_BUFFER_ALLOCATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn allocate_buffer(
    device: &MetalDevice,
    size: usize,
    options: MTLResourceOptions,
) -> Result<MetalBuffer, Error> {
    if size as u64 > device.max_buffer_length() {
        return Err(fail(OUT_OF_RANGE, "Buffer exceeds Metal maxBufferLength"));
    }
    unsafe {
        // metal-rs's new_buffer assumes a non-null result. Check BEFORE creating
        // its owning wrapper, for both public allocations and internal uploads.
        #[cfg(test)]
        let inject_failure = FAIL_NEXT_BUFFER_ALLOCATION.with(|flag| flag.replace(false));
        #[cfg(not(test))]
        let inject_failure = false;
        let pointer: *mut ::metal::MTLBuffer = if inject_failure {
            ptr::null_mut()
        } else {
            msg_send![device.as_ptr(), newBufferWithLength: size as u64 options: options]
        };
        if pointer.is_null() {
            return Err(fail(INTERNAL_ERROR, "Metal buffer allocation failed"));
        }
        Ok(MetalBuffer::from_ptr(pointer))
    }
}

impl Batch {
    fn new(device: Rc<Device>) -> Result<Self, Error> {
        unsafe {
            let allocator = Mtl4::owned(
                msg_send![device.raw.as_ptr(), newCommandAllocator],
                "command allocator",
            )?;
            let commands = Mtl4::owned(
                msg_send![device.raw.as_ptr(), newCommandBuffer],
                "command buffer",
            )?;
            let _: () = msg_send![commands.0, beginCommandBufferWithAllocator: allocator.0];
            let _: () = msg_send![commands.0, useResidencySet: device.residency.0];
            let encoder = Mtl4::retained(
                msg_send![commands.0, computeCommandEncoder],
                "compute encoder",
            )?;
            Ok(Self {
                device,
                recording: Some(Recording {
                    encoder: Some(encoder),
                    commands,
                    _allocator: allocator,
                }),
                retained: Vec::new(),
                transient: Vec::new(),
                tables: Vec::new(),
            })
        }
    }

    fn encoder(&mut self) -> Result<*mut Object, Error> {
        Ok(contract::recording(&mut self.recording)?
            .encoder
            .as_ref()
            .expect("open recording")
            .0)
    }

    fn argument_table(&mut self, addresses: &[u64]) -> Result<Mtl4, Error> {
        unsafe {
            let descriptor = Mtl4::owned(
                msg_send![class!(MTL4ArgumentTableDescriptor), new],
                "argument table descriptor",
            )?;
            let _: () = msg_send![descriptor.0, setMaxBufferBindCount: addresses.len()];
            let table = Mtl4::owned(
                msg_send![self.device.raw.as_ptr(), newArgumentTableWithDescriptor: descriptor.0 error: ptr::null_mut::<*mut Object>()],
                "argument table",
            )?;
            for (index, address) in addresses.iter().copied().enumerate() {
                let _: () = msg_send![table.0, setAddress: address atIndex: index];
            }
            Ok(table)
        }
    }

    fn upload(&mut self, bytes: &[u8]) -> Result<u64, Error> {
        let size = bytes.len().max(1);
        let buffer = allocate_buffer(
            &self.device.raw,
            size,
            MTLResourceOptions::StorageModeShared,
        )?;
        if !bytes.is_empty() {
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.contents().cast(), bytes.len())
            };
        }
        let address = buffer.gpu_address();
        unsafe {
            let _: () = msg_send![self.device.residency.0, addAllocation: buffer.as_ptr()];
            let _: () = msg_send![self.device.residency.0, commit];
        }
        self.transient.push(buffer);
        Ok(address)
    }

    fn release_transients(&mut self) {
        unsafe {
            for buffer in &self.transient {
                let _: () = msg_send![self.device.residency.0, removeAllocation: buffer.as_ptr()];
            }
            let _: () = msg_send![self.device.residency.0, commit];
        }
    }

    fn dispatch(
        &mut self,
        kernel: Rc<Kernel>,
        groups: [u32; 3],
        arguments: &[u8],
    ) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.device, &kernel.device) {
            return Err(fail(INVALID_ARGUMENT, "Kernel belongs to another device"));
        }
        contract::dispatch(
            groups,
            limits(&self.device.raw).max_dispatch,
            arguments.len(),
            kernel.push_size,
        )?;
        let encoder = self.encoder()?;
        let address = self.upload(arguments)?;
        let table = self.argument_table(&[address])?;
        unsafe {
            let pipeline = &*kernel.pipeline.as_ptr().cast::<Object>();
            let table_object = &*table.0;
            let _: () = msg_send![encoder, setComputePipelineState: pipeline];
            let _: () = msg_send![encoder, setArgumentTable: table_object];
            let grid = MTLSize {
                width: groups[0] as _,
                height: groups[1] as _,
                depth: groups[2] as _,
            };
            let _: () =
                msg_send![encoder, dispatchThreadgroups: grid threadsPerThreadgroup: kernel.group];
        }
        self.tables.push(table);
        Ok(())
    }

    fn submit(&mut self) -> Result<Completion, Error> {
        self.encoder()?;
        let feedback = Arc::new(feedback::Feedback::default());
        // Perform fallible preparation before consuming the recording.
        let options = feedback::options(feedback.clone())?;
        let mut recording = contract::take_recording(&mut self.recording)?;
        recording.end();
        let mut submission = contract::Submission::preparing(SubmissionResources {
            recording,
            _transient: std::mem::take(&mut self.transient),
            _tables: std::mem::take(&mut self.tables),
            _retained: std::mem::take(&mut self.retained),
        });
        unsafe {
            let buffers = [submission.resources.as_ref().unwrap().recording.commands.0];
            let _: () = msg_send![self.device.queue.0,
                commit: buffers.as_ptr() count: 1usize options: options.0];
        }
        submission.accept();
        Ok(Completion {
            _device: self.device.clone(),
            feedback,
            submission,
        })
    }
}

impl Drop for Batch {
    fn drop(&mut self) {
        drop(self.recording.take());
        self.release_transients();
    }
}

impl Completion {
    fn observe(&mut self, wait: bool) -> Result<bool, Error> {
        if self.submission.pending {
            // Feedback is delivered after this commit's workload terminates,
            // including failure. An event value alone cannot establish success.
            let Some(outcome) = self.feedback.observe(wait) else {
                return Ok(false);
            };
            if let Some(resources) = self.submission.resources.as_ref() {
                unsafe {
                    for buffer in &resources._transient {
                        let _: () =
                            msg_send![self._device.residency.0, removeAllocation: buffer.as_ptr()];
                    }
                    let _: () = msg_send![self._device.residency.0, commit];
                }
            }
            self.submission.finish(outcome);
        }
        self.submission
            .outcome
            .as_ref()
            .expect("accepted submission")
            .clone()?;
        Ok(true)
    }
}

impl Drop for Completion {
    fn drop(&mut self) {
        let _ = self.observe(true);
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_device_create(
    probe: *const OgpuProbe,
    index: u32,
    out: *mut *mut OgpuDevice,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out, error, || {
            required(probe)?;
            let probe = &*probe;
            let raw = probe
                .metal_devices
                .get(index as usize)
                .ok_or_else(|| fail(OUT_OF_RANGE, "Device index out of range"))?
                .to_owned();
            if !supported(&raw) {
                return Err(fail(
                    UNSUPPORTED,
                    "Metal execution requires Apple Silicon with Metal 4 (macOS 26+)",
                ));
            }
            Ok(OgpuDevice {
                inner: make_device(raw)?,
            })
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_device_create_graphics(
    _probe: *const OgpuProbe,
    _index: u32,
    out: *mut *mut OgpuDevice,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out, error, || {
            Err(fail(
                UNSUPPORTED,
                "Native Metal backend currently supports compute only",
            ))
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_device_destroy(p: *mut OgpuDevice) {
    if !p.is_null() {
        unsafe { drop(Box::from_raw(p)) }
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_device_capabilities(
    device: *const OgpuDevice,
    out: *mut OgpuCapabilities,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(device)?;
            required(out)?;
            out.write(capabilities());
            Ok(())
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_device_limits(
    device: *const OgpuDevice,
    out: *mut OgpuDeviceLimits,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(device)?;
            required(out)?;
            let device = &*device;
            out.write(limits(&device.inner.raw));
            Ok(())
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_device_timing_info(
    device: *mut OgpuDevice,
    out: *mut OgpuTimingInfo,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(out)?;
            out.write(OgpuTimingInfo::default());
            required(device)?;
            Err(fail(
                UNSUPPORTED,
                "Metal queue timestamps are not exposed by this backend",
            ))
        })
    }
}

#[no_mangle]
#[allow(unexpected_cfgs)] // objc 0.2's selector macro checks its historical cargo-clippy feature.
pub unsafe extern "C" fn ogpu_buffer_create(
    device: *mut OgpuDevice,
    size: u64,
    placement: u32,
    out: *mut *mut OgpuBuffer,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out, error, || {
            required(device)?;
            let size =
                usize::try_from(size).map_err(|_| fail(INVALID_ARGUMENT, "Buffer too large"))?;
            if size == 0 {
                return Err(fail(INVALID_ARGUMENT, "Buffer size must be nonzero"));
            }
            let host = match placement {
                0 => true,
                1 => false,
                _ => return Err(fail(INVALID_ARGUMENT, "Invalid buffer placement")),
            };
            let options = if host {
                MTLResourceOptions::StorageModeShared
            } else {
                MTLResourceOptions::StorageModePrivate
            };
            let device = &*device;
            let device_raw = &device.inner.raw;
            let raw = allocate_buffer(device_raw, size, options)?;
            let inner = Rc::new(Buffer {
                device: device.inner.clone(),
                raw,
                size,
                host,
            });
            inner
                .device
                .buffers
                .borrow_mut()
                .insert(Rc::as_ptr(&inner) as usize, Rc::downgrade(&inner));
            let _: () = msg_send![inner.device.residency.0, addAllocation: inner.raw.as_ptr()];
            let _: () = msg_send![inner.device.residency.0, commit];
            Ok(OgpuBuffer { inner })
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_buffer_destroy(p: *mut OgpuBuffer) {
    if !p.is_null() {
        unsafe { drop(Box::from_raw(p)) }
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_buffer_write(
    buffer: *mut OgpuBuffer,
    offset: u64,
    data: *const c_void,
    size: u64,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(buffer)?;
            let b = &(&(*buffer).inner);
            if !b.host {
                return Err(fail(
                    INVALID_ARGUMENT,
                    "Device buffer is not CPU accessible",
                ));
            }
            let (offset, size) = check_range(b.size, offset, size)?;
            if size != 0 {
                required(data)?;
                ptr::copy_nonoverlapping(
                    data.cast::<u8>(),
                    b.raw.contents().cast::<u8>().add(offset),
                    size,
                );
            }
            Ok(())
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_buffer_read(
    buffer: *const OgpuBuffer,
    offset: u64,
    data: *mut c_void,
    size: u64,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(buffer)?;
            let b = &(&(*buffer).inner);
            if !b.host {
                return Err(fail(
                    INVALID_ARGUMENT,
                    "Device buffer is not CPU accessible",
                ));
            }
            let (offset, size) = check_range(b.size, offset, size)?;
            if size != 0 {
                required(data)?;
                ptr::copy_nonoverlapping(
                    b.raw.contents().cast::<u8>().add(offset),
                    data.cast::<u8>(),
                    size,
                );
            }
            Ok(())
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_buffer_device_address(
    buffer: *const OgpuBuffer,
    out: *mut u64,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(buffer)?;
            required(out)?;
            let buffer = &*buffer;
            out.write(buffer.inner.raw.gpu_address());
            Ok(())
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_kernel_create(
    device: *mut OgpuDevice,
    desc: *const OgpuShaderDesc,
    push_size: u32,
    out: *mut *mut OgpuKernel,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out, error, || {
            required(device)?;
            let device = &*device;
            contract::root_size(push_size, limits(&device.inner.raw).max_push_data_bytes)?;
            let shader = crate::shader::Shader::read(desc)?;
            let (pipeline, group) = prepare(&device.inner.raw, shader)?;
            Ok(OgpuKernel {
                inner: Rc::new(Kernel {
                    device: device.inner.clone(),
                    pipeline,
                    group,
                    push_size,
                }),
            })
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_kernel_destroy(p: *mut OgpuKernel) {
    if !p.is_null() {
        unsafe { drop(Box::from_raw(p)) }
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_create(
    device: *mut OgpuDevice,
    out: *mut *mut OgpuBatch,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out, error, || {
            required(device)?;
            Ok(OgpuBatch {
                inner: Batch::new((*device).inner.clone())?,
            })
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_destroy(p: *mut OgpuBatch) {
    if !p.is_null() {
        unsafe { drop(Box::from_raw(p)) }
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_dispatch(
    batch: *mut OgpuBatch,
    kernel: *mut OgpuKernel,
    x: u32,
    y: u32,
    z: u32,
    args: *const c_void,
    bytes: u32,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(batch)?;
            required(kernel)?;
            if bytes != 0 {
                required(args)?;
            }
            let args = if bytes == 0 {
                &[]
            } else {
                std::slice::from_raw_parts(args.cast::<u8>(), bytes as usize)
            };
            (*batch)
                .inner
                .dispatch((*kernel).inner.clone(), [x, y, z], args)
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_dispatch_wait(
    kernel: *mut OgpuKernel,
    x: u32,
    y: u32,
    z: u32,
    args: *const c_void,
    bytes: u32,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(kernel)?;
            if bytes != 0 {
                required(args)?;
            }
            let kernel = &*kernel;
            let mut batch = Batch::new(kernel.inner.device.clone())?;
            let args = if bytes == 0 {
                &[]
            } else {
                std::slice::from_raw_parts(args.cast::<u8>(), bytes as usize)
            };
            batch.dispatch(kernel.inner.clone(), [x, y, z], args)?;
            batch.submit()?.observe(true)?;
            Ok(())
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_barrier(
    batch: *mut OgpuBatch,
    source: u32,
    destination: u32,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(batch)?;
            contract::access(source, false)?;
            contract::access(destination, false)?;
            let encoder = (*batch).inner.encoder()?;
            const DISPATCH: usize = 1 << 27;
            const BLIT: usize = 1 << 28;
            // All three scopes are necessary: earlier passes -> this/later
            // passes; earlier -> later commands here; this/earlier passes ->
            // later passes. Copies can lower to either blit or dispatch.
            let _: () = msg_send![encoder,
                barrierAfterQueueStages: DISPATCH | BLIT
                beforeStages: DISPATCH | BLIT
                visibilityOptions: 1usize];
            let _: () = msg_send![encoder,
                barrierAfterEncoderStages: DISPATCH | BLIT
                beforeEncoderStages: DISPATCH | BLIT
                visibilityOptions: 1usize];
            let _: () = msg_send![encoder,
                barrierAfterStages: DISPATCH | BLIT
                beforeQueueStages: DISPATCH | BLIT
                visibilityOptions: 1usize];
            Ok(())
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_retain_buffer(
    batch: *mut OgpuBatch,
    buffer: *mut OgpuBuffer,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(batch)?;
            required(buffer)?;
            let buffer = &*buffer;
            if !Rc::ptr_eq(&(*batch).inner.device, &buffer.inner.device) {
                return Err(fail(INVALID_ARGUMENT, "Buffer belongs to another device"));
            }
            (*batch).inner.encoder()?;
            contract::retain(&mut (*batch).inner.retained, buffer.inner.clone());
            Ok(())
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_copy_buffer(
    batch: *mut OgpuBatch,
    source: *mut OgpuBuffer,
    source_offset: u64,
    destination: *mut OgpuBuffer,
    destination_offset: u64,
    size: u64,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(batch)?;
            required(source)?;
            required(destination)?;
            let b = &mut (*batch).inner;
            let s = &(*source).inner;
            let d = &(*destination).inner;
            if !Rc::ptr_eq(&b.device, &s.device) || !Rc::ptr_eq(&b.device, &d.device) {
                return Err(fail(INVALID_ARGUMENT, "Buffer belongs to another device"));
            }
            b.encoder()?;
            let (so, doff, n) = contract::copy_ranges(
                s.size,
                source_offset,
                d.size,
                destination_offset,
                size,
                Rc::ptr_eq(s, d),
            )?;
            if n == 0 {
                return Ok(());
            }
            b.copy(s, so, d, doff, n)
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_enable_timing(
    batch: *mut OgpuBatch,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(batch)?;
            (*batch).inner.encoder()?;
            Err(fail(
                UNSUPPORTED,
                "Metal queue timestamps are not exposed by this backend",
            ))
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_submit(
    batch: *mut OgpuBatch,
    out: *mut *mut OgpuCompletion,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out, error, || {
            required(batch)?;
            Ok(OgpuCompletion {
                inner: (*batch).inner.submit()?,
            })
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_completion_wait(
    completion: *mut OgpuCompletion,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(completion)?;
            (*completion).inner.observe(true).map(|_| ())
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_completion_poll(
    completion: *mut OgpuCompletion,
    out: *mut u32,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(out)?;
            out.write(0);
            required(completion)?;
            out.write((*completion).inner.observe(false)? as u32);
            Ok(())
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_completion_elapsed_ns(
    completion: *mut OgpuCompletion,
    out: *mut f64,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(completion)?;
            required(out)?;
            out.write(0.0);
            Err(fail(INVALID_ARGUMENT, "Timing was not enabled"))
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_completion_destroy(p: *mut OgpuCompletion) {
    if !p.is_null() {
        unsafe { drop(Box::from_raw(p)) }
    }
}

fn unsupported_graphics() -> Result<(), Error> {
    Err(fail(
        UNSUPPORTED,
        "Native Metal backend currently supports the common compute subset only",
    ))
}

unsafe fn unsupported_create<T>(out: *mut *mut T, error: *mut OgpuError) -> OgpuResult {
    unsafe {
        create(out, error, || {
            Err(fail(
                UNSUPPORTED,
                "Native Metal backend currently supports the common compute subset only",
            ))
        })
    }
}

// Optional ABI-14/15 experiments. No native allocator or executable reuse is implied.
pub enum OgpuRecordingStorage {}
pub enum OgpuCommandList {}

#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_compile(
    batch: *mut OgpuBatch,
    out: *mut *mut OgpuCommandList,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out, error, || {
            required(batch)?;
            Err(fail(
                UNSUPPORTED,
                "Reusable command lists are not implemented on Metal",
            ))
        })
    }
}
#[no_mangle]
pub unsafe extern "C" fn ogpu_command_list_submit(
    list: *mut OgpuCommandList,
    out: *mut *mut OgpuCompletion,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out, error, || {
            required(list)?;
            Err(fail(
                UNSUPPORTED,
                "Reusable command lists are not implemented on Metal",
            ))
        })
    }
}
#[no_mangle]
pub unsafe extern "C" fn ogpu_command_list_destroy(_list: *mut OgpuCommandList) {}
fn unsupported_storage() -> Error {
    fail(
        UNSUPPORTED,
        "Explicit recording storage is not implemented on Metal",
    )
}
#[no_mangle]
pub unsafe extern "C" fn ogpu_recording_storage_create(
    device: *mut OgpuDevice,
    out: *mut *mut OgpuRecordingStorage,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out, error, || {
            required(device)?;
            Err(unsupported_storage())
        })
    }
}
#[no_mangle]
pub unsafe extern "C" fn ogpu_recording_storage_trim(
    storage: *mut OgpuRecordingStorage,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        call(error, || {
            required(storage)?;
            Err(unsupported_storage())
        })
    }
}
#[no_mangle]
pub unsafe extern "C" fn ogpu_recording_storage_destroy(_storage: *mut OgpuRecordingStorage) {}
#[no_mangle]
pub unsafe extern "C" fn ogpu_batch_create_in(
    storage: *mut OgpuRecordingStorage,
    out: *mut *mut OgpuBatch,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe {
        create(out, error, || {
            required(storage)?;
            Err(unsupported_storage())
        })
    }
}

macro_rules! unsupported_call {
    ($name:ident($($arg:ident: $ty:ty),* $(,)?)) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name($($arg: $ty,)* error: *mut OgpuError) -> OgpuResult {
            $(let _ = $arg;)*
            unsafe { call(error, unsupported_graphics) }
        }
    };
}

unsupported_call!(ogpu_image_check_support(device: *const OgpuDevice, desc: *const c_void));
unsupported_call!(ogpu_batch_discard_image(batch: *mut OgpuBatch, image: *const OgpuImage));
unsupported_call!(ogpu_batch_bind_image_heap(batch: *mut OgpuBatch, heap: *const OgpuImageHeap));
unsupported_call!(ogpu_image_heap_write(heap: *mut OgpuImageHeap, first: u32, entries: *const c_void, count: u32));
unsupported_call!(ogpu_image_heap_clear(heap: *mut OgpuImageHeap, first: u32, count: u32));
unsupported_call!(ogpu_sampler_heap_write(heap: *mut OgpuSamplerHeap, first: u32, entries: *const c_void, count: u32));
unsupported_call!(ogpu_batch_bind_sampler_heap(batch: *mut OgpuBatch, heap: *const OgpuSamplerHeap));
unsupported_call!(ogpu_batch_draw_indirect(batch: *mut OgpuBatch, raster: *mut OgpuRaster, image: *mut OgpuImage, indirect: *mut OgpuBuffer, offset: u64, arguments: *const c_void, argument_bytes: u32, load: u32));
unsupported_call!(ogpu_batch_copy_image_to_buffer(batch: *mut OgpuBatch, image: *mut OgpuImage, destination: *mut OgpuBuffer, offset: u64));
unsupported_call!(ogpu_batch_copy_buffer_to_image(batch: *mut OgpuBatch, source: *mut OgpuBuffer, offset: u64, image: *mut OgpuImage));

#[no_mangle]
pub unsafe extern "C" fn ogpu_image_heap_create(
    _device: *mut OgpuDevice,
    _capacity: u32,
    out: *mut *mut OgpuImageHeap,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe { unsupported_create(out, error) }
}
#[no_mangle]
pub unsafe extern "C" fn ogpu_sampler_heap_create(
    _device: *mut OgpuDevice,
    _capacity: u32,
    out: *mut *mut OgpuSamplerHeap,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe { unsupported_create(out, error) }
}
#[no_mangle]
pub unsafe extern "C" fn ogpu_image_create(
    _device: *mut OgpuDevice,
    _desc: *const c_void,
    out: *mut *mut OgpuImage,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe { unsupported_create(out, error) }
}
#[no_mangle]
pub unsafe extern "C" fn ogpu_raster_create(
    _device: *mut OgpuDevice,
    _vertex: *const OgpuShaderDesc,
    _fragment: *const OgpuShaderDesc,
    _push_size: u32,
    _topology: u32,
    _target_format: u32,
    out: *mut *mut OgpuRaster,
    error: *mut OgpuError,
) -> OgpuResult {
    unsafe { unsupported_create(out, error) }
}

#[no_mangle]
pub unsafe extern "C" fn ogpu_image_heap_destroy(_p: *mut OgpuImageHeap) {}
#[no_mangle]
pub unsafe extern "C" fn ogpu_sampler_heap_destroy(_p: *mut OgpuSamplerHeap) {}
#[no_mangle]
pub unsafe extern "C" fn ogpu_image_destroy(_p: *mut OgpuImage) {}
#[no_mangle]
pub unsafe extern "C" fn ogpu_raster_destroy(_p: *mut OgpuRaster) {}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    #[cfg(feature = "spirv-to-msl")]
    use crate::{OgpuSpecializationConstant, SUCCESS};
    #[test]
    fn ranges_are_checked() {
        assert_eq!(check_range(16, 16, 0).unwrap(), (16, 0));
        assert!(check_range(16, 16, 1).is_err());
        assert!(check_range(16, u64::MAX, 0).is_err());
    }

    #[test]
    #[cfg(feature = "spirv-to-msl")]
    #[ignore = "requires Apple Silicon Metal GPU and SPIR-V adapter"]
    fn specialized_spirv_executes_on_metal() {
        let raw = MetalDevice::system_default().expect("Metal GPU required");
        let device = make_device(raw).unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/shaders/specialize.comp.spv");
        let bytes = std::fs::read(path).unwrap();
        let words: Vec<u32> = bytes
            .chunks_exact(4)
            .map(|v| u32::from_le_bytes(v.try_into().unwrap()))
            .collect();
        let constants = [
            OgpuSpecializationConstant { id: 1, bits: 16 },
            OgpuSpecializationConstant {
                id: 2,
                bits: 2.0f32.to_bits(),
            },
            OgpuSpecializationConstant {
                id: 3,
                bits: (-3i32) as u32,
            },
            OgpuSpecializationConstant { id: 4, bits: 4 },
            OgpuSpecializationConstant { id: 5, bits: 0 },
        ];
        let desc = OgpuShaderDesc {
            code: words.as_ptr().cast(),
            entry_point: ptr::null(),
            format: crate::SHADER_SPIRV,
            local_size: [0; 3],
            code_size: (words.len() * 4) as u64,
            constants: constants.as_ptr(),
            constant_count: constants.len() as u32,
            reserved: 0,
        };
        let mut device_handle = OgpuDevice { inner: device };
        let mut buffer = ptr::null_mut();
        let mut kernel = ptr::null_mut();
        let mut error = OgpuError {
            vulkan_result: 0,
            message: [0; 256],
        };
        unsafe {
            assert_eq!(
                ogpu_buffer_create(&mut device_handle, 16, 0, &mut buffer, &mut error),
                SUCCESS
            );
            let status = ogpu_kernel_create(&mut device_handle, &desc, 8, &mut kernel, &mut error);
            let message = std::ffi::CStr::from_ptr(error.message.as_ptr()).to_string_lossy();
            assert_eq!(status, SUCCESS, "{message}");
            let mut address = 0;
            assert_eq!(
                ogpu_buffer_device_address(buffer, &mut address, &mut error),
                SUCCESS
            );
            assert_eq!(
                ogpu_dispatch_wait(
                    kernel,
                    1,
                    1,
                    1,
                    (&address as *const u64).cast(),
                    8,
                    &mut error
                ),
                SUCCESS
            );
            let mut result = [0u32; 4];
            assert_eq!(
                ogpu_buffer_read(buffer, 0, result.as_mut_ptr().cast(), 16, &mut error),
                SUCCESS
            );
            assert_eq!(result, [16, 2.0f32.to_bits(), 256.0f32.to_bits(), 0]);
            ogpu_kernel_destroy(kernel);
            ogpu_buffer_destroy(buffer);
        }
    }
}

fn prepare(
    device: &MetalDevice,
    shader: crate::shader::Shader<'_>,
) -> Result<(ComputePipelineState, MTLSize), Error> {
    debug_assert!(shader.format == crate::SHADER_SPIRV || shader.constants.is_empty());
    let (library, entry, local, constants) = match shader.format {
        crate::SHADER_SPIRV => {
            #[cfg(feature = "spirv-to-msl")]
            {
                let translated = spirv::translate(shader.spirv()?, shader.constants)?;
                let library = device
                    .new_library_with_source(&translated.source, &::metal::CompileOptions::new())
                    .map_err(|e| {
                        fail(
                            UNSUPPORTED,
                            format!("Translated MSL compilation failed: {e}"),
                        )
                    })?;
                (library, "main0", translated.local, translated.constants)
            }
            #[cfg(not(feature = "spirv-to-msl"))]
            return Err(fail(
                UNSUPPORTED,
                "SPIR-V to MSL adapter disabled; supply MSL or metallib",
            ));
        }
        crate::SHADER_MSL => {
            let source = std::str::from_utf8(shader.code)
                .map_err(|_| fail(INVALID_ARGUMENT, "MSL source is not UTF-8"))?;
            if source.contains('\0') {
                return Err(fail(INVALID_ARGUMENT, "MSL contains NUL"));
            }
            let library = device
                .new_library_with_source(source, &::metal::CompileOptions::new())
                .map_err(|e| fail(UNSUPPORTED, format!("MSL compilation failed: {e}")))?;
            (
                library,
                shader.entry,
                shader.local_size,
                Vec::<(u32, u32, ::metal::MTLDataType)>::new(),
            )
        }
        crate::SHADER_METALLIB => {
            let library = device
                .new_library_with_data(shader.code)
                .map_err(|e| fail(UNSUPPORTED, format!("Metal library loading failed: {e}")))?;
            (
                library,
                shader.entry,
                shader.local_size,
                Vec::<(u32, u32, ::metal::MTLDataType)>::new(),
            )
        }
        _ => unreachable!("shared shader validation"),
    };
    let values = FunctionConstantValues::new();
    for (id, bits, data_type) in &constants {
        if *data_type == ::metal::MTLDataType::Bool {
            let value = *bits != 0;
            values.set_constant_value_at_index(
                (&value as *const bool).cast(),
                *data_type,
                *id as u64,
            );
        } else {
            values.set_constant_value_at_index((bits as *const u32).cast(), *data_type, *id as u64);
        }
    }
    let function = library
        .get_function(entry, Some(values))
        .map_err(|e| fail(UNSUPPORTED, format!("Metal entry point failed: {e}")))?;
    let pipeline = device
        .new_compute_pipeline_state_with_function(&function)
        .map_err(|e| fail(UNSUPPORTED, format!("Metal pipeline creation failed: {e}")))?;
    let limits = limits(device);
    contract::dispatch(local, limits.max_group_size, 0, 0)?;
    let invocations = local
        .into_iter()
        .try_fold(1u64, |n, axis| n.checked_mul(u64::from(axis)));
    if invocations.is_none_or(|n| n > pipeline.max_total_threads_per_threadgroup())
        || pipeline.static_threadgroup_memory_length() > limits.max_shared_memory_bytes as u64
    {
        return Err(fail(
            INVALID_ARGUMENT,
            "Kernel exceeds Metal threadgroup limits",
        ));
    }
    Ok((
        pipeline,
        MTLSize {
            width: local[0] as _,
            height: local[1] as _,
            depth: local[2] as _,
        },
    ))
}

fn supported(device: &MetalDevice) -> bool {
    let has_metal4: bool =
        unsafe { msg_send![device.as_ptr(), respondsToSelector: sel!(newMTL4CommandQueue)] };
    device.supports_family(::metal::MTLGPUFamily::Apple7) && has_metal4
}
