#![cfg(target_os = "linux")]
#[test]
#[ignore = "requires Wayland session, GLFW 3.4 and optional Vulkan presentation profile"]
fn gpu_wayland_surface_setup() {
    // GLFW requires the process's main thread, not a Rust test worker. Launch
    // the independent consumer; do not call its window lifecycle in this thread.
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/foundation_rust/Cargo.toml");
    let output = std::process::Command::new(env!("CARGO"))
        .args(["run", "--locked", "--offline", "--manifest-path"])
        .arg(manifest)
        .args(["--bin", "surface"])
        .output()
        .expect("run standalone Wayland consumer");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("{stdout}{stderr}");
    assert!(output.status.success(), "surface probe failed");
    assert!(stdout.contains("SURFACE_SETUP_PASS"));
    for marker in ["Validation Error", "VUID-", "SYNC-HAZARD"] {
        assert!(
            !stdout.contains(marker) && !stderr.contains(marker),
            "native validation: {marker}"
        );
    }
}
