#![cfg(target_os = "linux")]
#[path = "../../../examples/foundation_rust/src/bin/artifacts.rs"]
mod consumer;
#[test]
#[ignore = "requires native pipeline binaries; capture/import/source-free GPU equivalence"]
fn gpu_pipeline_binary_artifacts() {
    consumer::run().unwrap();
}
