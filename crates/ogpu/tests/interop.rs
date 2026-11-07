#![cfg(target_os = "linux")]
#[path = "../../../examples/foundation_rust/src/bin/native.rs"]
mod consumer;
#[test]
#[ignore = "requires Vulkan; native producer to OGPU with explicit borrowed/owned handles"]
fn gpu_native_interop() {
    consumer::run().unwrap();
}
