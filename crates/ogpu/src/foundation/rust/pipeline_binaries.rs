//! Native binary storage; handles are explicit, non-retaining tokens.
use super::*;
use crate::foundation::{Executable, PipelineBinary};
use native::pipeline_binaries as b;
/// # Safety
/// Live device and no concurrent destruction.
pub unsafe fn pipeline_binary_identity(device: DeviceHandle) -> Result<ArtifactKey> {
    b::identity(unsafe { device.get() })
}
/// # Safety
/// Live device, intact native key/data provenance. Device outlives returned binary.
pub unsafe fn pipeline_binary_import(
    device: DeviceHandle,
    identity: &ArtifactKey,
    key: &ArtifactKey,
    data: &[u8],
) -> Result<PipelineBinaryHandle> {
    let p = unsafe {
        PipelineBinary::import(
            device.get(),
            identity,
            key,
            Bytes {
                data: data.as_ptr().cast(),
                size: data.len(),
            },
        )?
    };
    Ok(PipelineBinaryHandle(NonNull::from(Box::leak(p))))
}
/// # Safety
/// Live binary/device, no concurrent destruction, disjoint output.
pub unsafe fn pipeline_binary_export(
    binary: PipelineBinaryHandle,
    key: &mut ArtifactKey,
    size: &mut usize,
    data: Option<&mut [u8]>,
) -> Result<()> {
    let out = match data {
        None => std::ptr::null_mut(),
        Some(bytes) => {
            *size = bytes.len();
            bytes.as_mut_ptr().cast()
        }
    };
    unsafe { binary.get().export(key, size, out) }
}
/// # Safety
/// Live uniquely destroyed binary, no concurrent use. Never waits.
pub unsafe fn pipeline_binary_destroy(binary: PipelineBinaryHandle) {
    drop(unsafe { Box::from_raw(binary.as_raw()) });
}
/// # Safety
/// Live executable/device, exclusive against release/destruction; disjoint outputs.
/// Success transfers each non-null output to the caller. None queries count only.
pub unsafe fn executable_binary_capture(
    executable: ExecutableHandle,
    output: Option<&mut [*mut PipelineBinary]>,
    total: &mut u32,
) -> Result<()> {
    let (capacity, out) = match output {
        None => (0, std::ptr::null_mut()),
        Some(o) => (count(o.len())?, o.as_mut_ptr()),
    };
    unsafe { b::capture(executable.get(), capacity, out, total) }
}
/// # Safety
/// Live executable/device, exclusive capture/release/destruction access.
pub unsafe fn executable_capture_release(executable: ExecutableHandle) -> Result<()> {
    b::release(unsafe { executable.get() })
}
/// # Safety
/// Same descriptor/layout/artifact obligations as executable_create; source shaders
/// are required. No pipeline is created and no compilation fallback occurs.
pub unsafe fn executable_key(
    device: DeviceHandle,
    desc: &ExecutableDesc,
    layout: ArgumentLayoutHandle,
) -> Result<ArtifactKey> {
    let mut key = ArtifactKey::default();
    unsafe {
        Executable::prepare(device.get(), desc, layout.get(), b::Action::Key(&mut key))?;
    }
    Ok(key)
}
/// # Safety
/// Same source descriptor/layout obligations as executable_key; disjoint output.
/// Success transfers each returned binary. A miss never compiles.
pub unsafe fn executable_binary_lookup(
    device: DeviceHandle,
    desc: &ExecutableDesc,
    layout: ArgumentLayoutHandle,
    output: Option<&mut [*mut PipelineBinary]>,
    total: &mut u32,
) -> Result<()> {
    let (capacity, out) = match output {
        None => (0, std::ptr::null_mut()),
        Some(o) => (count(o.len())?, o.as_mut_ptr()),
    };
    *total = 0;
    unsafe {
        b::clear_outputs(capacity, out)?;
        Executable::prepare(
            device.get(),
            desc,
            layout.get(),
            b::Action::Lookup {
                capacity,
                out,
                count: total,
            },
        )?;
    }
    Ok(())
}
