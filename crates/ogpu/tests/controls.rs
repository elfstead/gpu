#![cfg(target_os = "linux")]
#[path = "../../../examples/foundation_rust/src/bin/controls.rs"]
mod consumer;
#[test]
#[ignore = "requires native conditional execution and primitive queries"]
fn gpu_execution_controls() {
    consumer::run().unwrap();
}
