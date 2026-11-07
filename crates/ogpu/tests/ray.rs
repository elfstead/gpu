#![cfg(target_os = "linux")]
#[path = "../../../examples/foundation_ray/main.rs"]
mod consumer;
#[test]
#[ignore = "requires advertised AS/ray-query support; explicit RADV emulate_rt is diagnostic only"]
fn gpu_ray_query_consumer() {
    consumer::run().unwrap();
}

#[test]
#[ignore = "requires native ray pipeline and both indirect profiles; emulation is diagnostic only"]
fn gpu_ray_pipeline_consumer() {
    consumer::run_profile(true).unwrap();
}

#[test]
#[ignore = "requires AS/ray queries; emulated procedural traversal is diagnostic only"]
fn gpu_ray_procedural_query() {
    consumer::run_geometry(false, true).unwrap();
}
#[test]
#[ignore = "requires ray pipelines; emulated procedural traversal is diagnostic only"]
fn gpu_ray_procedural_pipeline() {
    consumer::run_geometry(true, true).unwrap();
}
