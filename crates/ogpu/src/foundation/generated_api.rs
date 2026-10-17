//! Generated-command C boundary. Validation/execution live below both host APIs.
use super::*;
/// # Safety
/// Valid versioned description/tokens, live device, disjoint writable output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_generated_layout_create(
    device: *mut Device,
    desc: *const GeneratedLayoutDesc,
    out: *mut *mut GeneratedLayout,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let d = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let desc = unsafe { native::executable_record(desc.cast(), GENERATED_LAYOUT_DESC)? };
        unsafe {
            out.write(rust::generated_layout_create(d, desc)?.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// Valid versioned description, live device/executable and disjoint writable output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_generated_set_create(
    device: *mut Device,
    desc: *const GeneratedSetDesc,
    out: *mut *mut GeneratedSet,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let d = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let desc = unsafe { native::executable_record(desc.cast(), GENERATED_SET_DESC)? };
        unsafe {
            out.write(rust::generated_set_create(d, desc)?.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// Exclusive live set, readable writes and live compatible executables; no pending updated slots.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_generated_set_update(
    set: *mut GeneratedSet,
    count: u32,
    writes: *const GeneratedSetWrite,
) -> Status {
    boundary(|| {
        let set = unsafe { rust::GeneratedSetHandle::from_raw(set) }.ok_or(INVALID)?;
        if count != 0 && writes.is_null() {
            return Err(INVALID);
        }
        let writes = if count == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(writes, count as usize) }
        };
        unsafe { rust::generated_set_update(set, writes) }
    })
}
/// # Safety
/// Live referenced objects; readable work and disjoint writable output; excludes set updates.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_generated_requirements(
    device: *mut Device,
    work: *const GeneratedWork,
    out: *mut GeneratedRequirements,
) -> Status {
    boundary(|| {
        let d = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let work = unsafe { work.as_ref() }.ok_or(INVALID)?;
        if out.is_null() {
            return Err(INVALID);
        }
        unsafe {
            out.write(rust::generated_requirements(d, work)?);
        }
        Ok(())
    })
}
/// # Safety
/// No recorded future/pending uses. Parent device remains live. NULL is a no-op.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_generated_layout_destroy(layout: *mut GeneratedLayout) {
    if let Some(layout) = unsafe { rust::GeneratedLayoutHandle::from_raw(layout) } {
        unsafe {
            rust::generated_layout_destroy(layout);
        }
    }
}
/// # Safety
/// No recorded future/pending uses. Borrowed initial executable/device remain live.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_generated_set_destroy(set: *mut GeneratedSet) {
    if let Some(set) = unsafe { rust::GeneratedSetHandle::from_raw(set) } {
        unsafe {
            rust::generated_set_destroy(set);
        }
    }
}
/// # Safety
/// Exclusive recording pools, live batch and state encoder. Storage/synchronization
/// and native state compatibility satisfy the header's generated-command contract.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_generated_preprocess(
    encoder: *mut List,
    batch: *const GeneratedBatch,
    state: *mut List,
) {
    unsafe {
        commands_api::encode(encoder, |e| {
            e.generated_preprocess(
                batch.as_ref().ok_or(INVALID)?,
                state.as_ref().ok_or(INVALID)?,
            )
        });
    }
}
/// # Safety
/// Live batch/resources, valid GPU records/counts and prepared storage at execution.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_generated_execute(
    encoder: *mut List,
    batch: *const GeneratedBatch,
    preprocessed: u32,
) {
    unsafe {
        commands_api::encode(encoder, |e| {
            e.generated_execute(batch.as_ref().ok_or(INVALID)?, preprocessed)
        });
    }
}
