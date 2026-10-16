#![cfg(target_os = "linux")]
#[test]
#[ignore = "requires Wayland session, GLFW 3.4 and optional Vulkan presentation profile"]
fn gpu_wayland_surface_setup() {
    probe(None, "SURFACE_SETUP_PASS");
}
#[test]
#[ignore = "requires live Wayland session, GLFW 3.4 and optional Vulkan presentation profile; opens two bounded windows"]
fn gpu_wayland_present_lifecycle() {
    probe(Some("--present"), "PRESENTATION_PASS");
}
#[test]
#[ignore = "requires live Wayland, GLFW and Vulkan presentation/raster; opens a bounded textured scene"]
fn gpu_wayland_textured_scene() {
    probe(Some("--scene-check"), "SCENE_PASS");
    probe(Some("--scene-smoke"), "SCENE_PASS");
}
fn probe(argument: Option<&str>, marker: &str) {
    // GLFW requires the process's main thread, not a Rust test worker. Launch
    // the independent consumer; do not call its window lifecycle in this thread.
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/foundation_rust/Cargo.toml");
    // Native WSI may block under compositor policy (e.g. occluded FIFO). Bound
    // the entire process group, including Cargo's child, rather than hanging CI.
    let mut command = std::process::Command::new("timeout");
    command.args(["--kill-after=2s", "30s", env!("CARGO")]);
    command
        .args(["run", "--locked", "--offline", "--manifest-path"])
        .arg(manifest)
        .args(["--bin", "surface"]);
    if let Some(argument) = argument {
        command.args(["--", argument]);
    }
    let output = command.output().expect("run standalone Wayland consumer");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("{stdout}{stderr}");
    assert!(output.status.success(), "surface probe failed");
    assert!(stdout.contains(marker));
    for marker in ["Validation Error", "VUID-", "SYNC-HAZARD"] {
        assert!(
            !stdout.contains(marker) && !stderr.contains(marker),
            "native validation: {marker}"
        );
    }
}
