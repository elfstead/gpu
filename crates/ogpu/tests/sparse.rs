#![cfg(target_os = "linux")]
#[path = "../../../examples/foundation_rust/src/bin/sparse.rs"]
mod consumer;
#[test]
#[ignore = "requires native Vulkan sparse binding"]
fn gpu_sparse_memory() {
    consumer::run().unwrap();
}
