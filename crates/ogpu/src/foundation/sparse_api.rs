//! C adapters for explicit page binding.
use super::*;
/// # Safety
/// Live device, readable description and writable query storage.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_image_sparse_requirements(
    device: *const Device,
    desc: *const ImageDesc,
    query: *mut Query,
) -> Status {
    boundary(|| {
        let device = unsafe { rust::DeviceHandle::from_raw(device.cast_mut()) }.ok_or(INVALID)?;
        if desc.is_null() {
            return Err(INVALID);
        }
        unsafe { desc.cast::<Record>().read() }.validate::<ImageDesc>(IMAGE_DESC)?;
        let query = unsafe { query.as_mut() }.ok_or(INVALID)?;
        unsafe { rust::image_sparse_requirements(device, &*desc, query) }
    })
}
/// # Safety
/// Live device, readable nested arrays and writable output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_sparse_scratch_requirements(
    device: *const Device,
    count: u32,
    descriptions: *const SparseSubmit,
    out: *mut HostRequirements,
) -> Status {
    boundary(|| {
        let device = unsafe { rust::DeviceHandle::from_raw(device.cast_mut()) }.ok_or(INVALID)?;
        let out = unsafe { out.as_mut() }.ok_or(INVALID)?;
        *out = unsafe {
            rust::sparse_scratch_requirements(device, native::command_slice(descriptions, count)?)?
        };
        Ok(())
    })
}
/// # Safety
/// Exclusive live queue and nested resources; readable arrays, writable count
/// outcomes, disjoint scratch; caller proves ordering/lifetime/aliasing legality.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_queue_bind_sparse(
    queue: *mut Queue,
    count: u32,
    descriptions: *const SparseSubmit,
    outcomes: *mut SubmitOutcome,
    scratch: HostSpan,
) -> Status {
    if count != 0 && outcomes.is_null() {
        return INVALID;
    }
    boundary(|| {
        let result = (|| {
            let queue = unsafe { queue.as_ref() }.ok_or(INVALID)?;
            let descriptions = unsafe { native::command_slice(descriptions, count)? };
            unsafe { queue.bind_sparse_to(descriptions, outcomes, count as usize, scratch) }
        })();
        // Engine writes outcomes itself. Pre-engine pointer failures reject all.
        if queue.is_null() || (count != 0 && descriptions.is_null()) {
            let outcome = SubmitOutcome {
                status: INVALID,
                acceptance: SUBMIT_REJECTED,
            };
            for i in 0..count as usize {
                unsafe {
                    outcomes.add(i).write(outcome);
                }
            }
        }
        result
    })
}
