#![cfg(target_os = "linux")]
#[path = "../../../examples/foundation_rust/src/bin/external.rs"]
mod consumer;
#[test]
#[ignore = "requires Linux opaque-FD memory and timeline sharing on one physical GPU"]
fn gpu_external_fd() {
    consumer::run().unwrap();
}
