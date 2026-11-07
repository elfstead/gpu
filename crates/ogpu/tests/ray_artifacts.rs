#![cfg(target_os = "linux")]
#[path = "../../../examples/foundation_rust/src/bin/ray_artifacts.rs"]
mod consumer;
#[test]
#[ignore = "requires ray libraries; emulation must be externally selected and reported separately"]
fn gpu_ray_libraries_replay() {
    consumer::run().unwrap();
}
