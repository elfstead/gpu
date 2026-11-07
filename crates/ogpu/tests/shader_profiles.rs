#![cfg(target_os = "linux")]
#[path = "../../../examples/foundation_rust/src/bin/shader_profiles.rs"]
mod consumer;
#[test]
#[ignore = "requires native subgroup rotation, compute derivatives and narrow push storage"]
fn gpu_shader_profile_artifacts() {
    consumer::run().unwrap();
}
