//! Descriptor encoding uses caller host bytes; heap binding uses caller GPU spans.
use super::*;

/// # Safety
/// Writable requirements output, disjoint from inputs. No GPU device needed.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_descriptor_scratch_requirements(
    kind: u32,
    count: u32,
    out: *mut HostRequirements,
) -> Status {
    boundary(|| {
        if out.is_null() {
            return Err(INVALID);
        }
        let requirements = native::descriptor_scratch(kind, count)?;
        unsafe {
            out.write(requirements);
        }
        Ok(())
    })
}
/// # Safety
/// See header: live device/resources, readable arrays and disjoint writable bytes/scratch.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_write_resource_descriptors(
    device: *const Device,
    count: u32,
    inputs: *const ResourceDescriptor,
    outputs: *const HostSpan,
    scratch: HostSpan,
) -> Status {
    boundary(|| {
        unsafe { device.as_ref() }
            .ok_or(INVALID)
            .and_then(|d| unsafe { d.write_resources(count, inputs, outputs, scratch) })
    })
}
/// # Safety
/// See header: live device, readable arrays and disjoint writable bytes/scratch.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_write_sampler_descriptors(
    device: *const Device,
    count: u32,
    inputs: *const SamplerDesc,
    outputs: *const HostSpan,
    scratch: HostSpan,
) -> Status {
    boundary(|| {
        unsafe { device.as_ref() }
            .ok_or(INVALID)
            .and_then(|d| unsafe { d.write_samplers(count, inputs, outputs, scratch) })
    })
}
