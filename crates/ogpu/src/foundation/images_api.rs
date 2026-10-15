//! C image boundary: independently owned memory, image definitions and views.
use super::*;

unsafe fn description<'a, T>(pointer: *const T, kind: u32) -> Result<&'a T, Status> {
    if pointer.is_null() {
        return Err(INVALID);
    }
    unsafe { pointer.cast::<Record>().read() }.validate::<T>(kind)?;
    Ok(unsafe { &*pointer })
}
/// # Safety
/// Live device, readable profile and disjoint writable limits output.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_image_limits_query(
    device: *const Device,
    profile: *const ImageProfile,
    out: *mut ImageLimits,
) -> Status {
    boundary(|| {
        let d = unsafe { rust::DeviceHandle::from_raw(device.cast_mut()) }.ok_or(INVALID)?;
        if out.is_null() {
            return Err(INVALID);
        }
        let limits = unsafe { rust::image_limits_query(d, description(profile, IMAGE_PROFILE)?)? };
        unsafe {
            out.write(limits);
        }
        Ok(())
    })
}
/// # Safety
/// Live device, readable description/arrays, writable output as in ogpu_next.h.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_image_requirements(
    device: *const Device,
    desc: *const ImageDesc,
    out: *mut Requirements,
) -> Status {
    boundary(|| {
        let d = unsafe { rust::DeviceHandle::from_raw(device.cast_mut()) }.ok_or(INVALID)?;
        let out = unsafe { out.as_mut() }.ok_or(INVALID)?;
        unsafe { rust::image_requirements(d, description(desc, IMAGE_DESC)?, out) }
    })
}
/// # Safety
/// Device remains live until image destruction; description and output are valid.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_image_create_unbound(
    device: *mut Device,
    desc: *const ImageDesc,
    out: *mut *mut Image,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let d = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let image = unsafe { rust::image_create_unbound(d, description(desc, IMAGE_DESC)?)? };
        unsafe {
            out.write(image.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// Same as unbound creation; placement is valid and outlives the image and its uses.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_image_create(
    device: *mut Device,
    desc: *const ImageDesc,
    placement: Span,
    out: *mut *mut Image,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let d = unsafe { rust::DeviceHandle::from_raw(device) }.ok_or(INVALID)?;
        let image = unsafe { rust::image_create(d, description(desc, IMAGE_DESC)?, placement)? };
        unsafe {
            out.write(image.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// Unbound live image, exclusive host access, and placement that outlives its uses.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_image_bind(image: *mut Image, placement: Span) -> Status {
    boundary(|| unsafe {
        rust::image_bind(
            rust::ImageHandle::from_raw(image).ok_or(INVALID)?,
            placement,
        )
    })
}
/// # Safety
/// Live unbound image and writable output. Memory and image remain separate owners.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_memory_create_dedicated_image(
    image: *mut Image,
    memory_type: u32,
    out: *mut *mut Memory,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let image = unsafe { rust::ImageHandle::from_raw(image) }.ok_or(INVALID)?;
        let memory = unsafe { rust::memory_create_dedicated_image(image, memory_type)? };
        unsafe {
            out.write(memory.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// No remaining views or recorded/pending uses, and live device. NULL permitted.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_image_destroy(image: *mut Image) {
    if let Some(image) = unsafe { rust::ImageHandle::from_raw(image) } {
        unsafe {
            rust::image_destroy(image);
        }
    }
}
/// # Safety
/// Live bound image; image/backing/device outlive this view and its uses.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_view_create(
    image: *mut Image,
    desc: *const ViewDesc,
    out: *mut *mut View,
) -> Status {
    if out.is_null() {
        return INVALID;
    }
    unsafe {
        out.write(ptr::null_mut());
    }
    boundary(|| {
        let image = unsafe { rust::ImageHandle::from_raw(image) }.ok_or(INVALID)?;
        let view = unsafe { rust::view_create(image, description(desc, VIEW_DESC)?)? };
        unsafe {
            out.write(view.as_raw());
        }
        Ok(())
    })
}
/// # Safety
/// No recorded/pending uses; parent image/device remain live. NULL permitted.
#[no_mangle]
pub unsafe extern "C" fn ogpu_next_view_destroy(view: *mut View) {
    if let Some(view) = unsafe { rust::ViewHandle::from_raw(view) } {
        unsafe {
            rust::view_destroy(view);
        }
    }
}
