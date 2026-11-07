#![cfg(target_os = "linux")]
#[path = "../../../examples/foundation_ray/main.rs"]
mod consumer;
#[test]
#[ignore = "requires advertised AS/ray-query support; explicit RADV emulate_rt is diagnostic only"]
fn gpu_ray_query_consumer() {
    consumer::run().unwrap();
}
