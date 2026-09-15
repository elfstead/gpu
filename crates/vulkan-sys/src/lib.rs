//! Private, generated Vulkan ABI declarations. Regenerate with `cargo xtask bindings`.
#![allow(warnings)]

#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64")
)))]
compile_error!("The generated ABI declarations compile only on Linux x86-64 and macOS arm64; regenerate and validate before adding another target.");

include!("bindings.rs");
