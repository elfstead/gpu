#![cfg(target_os = "linux")]
#[path = "../../../examples/foundation_rust/src/main.rs"]
mod consumer;

#[test]
#[ignore = "requires modern Vulkan; native Rust consumer, no C adapter calls"]
fn gpu_native_rust_consumer() {
    consumer::run().unwrap();
}
