//! Private, generated Vulkan ABI declarations. Regenerate with `cargo xtask bindings`.
#![allow(warnings)]

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
compile_error!("The initial bindings are validated only for Linux x86-64; regenerate and validate before adding another target.");

include!("bindings.rs");
