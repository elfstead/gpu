#![cfg(target_os = "linux")]
#[test]
#[ignore = "requires Wayland session, GLFW 3.4 and optional Vulkan presentation profile"]
fn gpu_wayland_surface_setup() {
    probe(false);
}
#[test]
#[ignore = "requires live Wayland session, GLFW 3.4 and optional Vulkan presentation profile; opens two bounded windows"]
fn gpu_wayland_present_lifecycle() {
    probe(true);
}
fn probe(present: bool) {
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
    if present {
        command.args(["--", "--present"]);
    }
    let output = command.output().expect("run standalone Wayland consumer");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("{stdout}{stderr}");
    assert!(output.status.success(), "surface probe failed");
    assert!(stdout.contains(if present {
        "PRESENTATION_PASS"
    } else {
        "SURFACE_SETUP_PASS"
    }));
    for marker in ["Validation Error", "VUID-", "SYNC-HAZARD"] {
        assert!(
            !stdout.contains(marker) && !stderr.contains(marker),
            "native validation: {marker}"
        );
    }
}
