//! App-local D3D12 Agility SDK deployment markers (Windows only).
//!
//! The D3D12 loader reads these exported symbols from the host executable to
//! pin the exact Agility runtime and SDK-layer directory. Keeping the runtime
//! app-local gives reproducible debug-layer behavior independent of the OS
//! optional components. Version must match the deployed `D3D12Core.dll` and
//! `d3d12SDKLayers.dll` (Microsoft.Direct3D.D3D12 1.613.1).

/// Exact Agility SDK release deployed with the runtime (1.613.x).
#[no_mangle]
pub static D3D12SDKVersion: u32 = 613;

/// Pointer-sized export value; the documented ABI is
/// `extern const char *D3D12SDKPath`, i.e. an exported pointer variable.
#[repr(transparent)]
pub struct PathPointer(*const u8);
// SAFETY: the export is immutable read-only data consumed by the D3D12 loader.
unsafe impl Sync for PathPointer {}

/// Directory of the app-local D3D12 runtime, relative to the executable.
#[no_mangle]
pub static D3D12SDKPath: PathPointer = PathPointer(b".\\D3D12\\\0".as_ptr());

/// Embedded linker directive: export the markers as data symbols. The
/// directive lives in this object so it applies exactly when these markers are
/// linked into a binary, and never to unrelated empty test executables.
#[used]
#[link_section = ".drectve"]
static DX12_SDK_EXPORTS: [u8; 55] = *b"/EXPORT:D3D12SDKVersion,DATA /EXPORT:D3D12SDKPath,DATA ";

/// Reference anchor: binaries that create D3D12 devices always pull this
/// object, so the loader finds the deployment markers.
pub(crate) fn touch() {
    std::hint::black_box((&D3D12SDKVersion, &D3D12SDKPath));
}
