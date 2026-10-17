#!/usr/bin/env python3
"""Relocate only the native Rust source bundle; build fresh, then optionally execute."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile


def require(value, message):
    if not value: raise ValueError(message)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prefix", required=True, type=Path)
    parser.add_argument("--no-gpu", action="store_true")
    parser.add_argument("--presentation", action="store_true", help="also open bounded Wayland windows")
    args = parser.parse_args()
    require(not (args.no_gpu and args.presentation), "--presentation conflicts with --no-gpu")
    prefix = args.prefix.resolve()
    manifest = json.loads((prefix / "share/ogpu/manifest.json").read_text())
    description = manifest["native_rust"]
    require(description["path"] == "share/ogpu/native-rust", "unknown bundle layout")
    source = prefix / description["path"]
    expected = {name.removeprefix(description["path"] + "/"): digest
                for name, digest in manifest["files_sha256"].items()
                if name.startswith(description["path"] + "/")}
    require(expected, "missing source hashes")
    for name, digest in expected.items():
        file = source / name
        require(file.resolve().is_relative_to(source), "source path escaped bundle")
        require(hashlib.sha256(file.read_bytes()).hexdigest() == digest, f"modified source: {name}")
    actual = {str(p.relative_to(source)) for p in source.rglob("*") if p.is_file()}
    require(actual == expected.keys(), "unexpected/missing bundle files")
    temporary = Path(tempfile.mkdtemp(prefix="ogpu-rust-external-"))
    bundle = temporary / "independent Rust source"
    shutil.copytree(source, bundle)
    identity = json.loads((bundle / "source.json").read_text())
    require(identity["revision"] == manifest["revision"] and identity["source_dirty"] == manifest["source_dirty"],
            "source identity mismatch")
    report = dict(source=identity, complete=False, build=False, runs=[], human_acceptance="unverified")
    print(f"Rust external artifacts: {temporary}", flush=True)
    environment = os.environ.copy()
    for name in ("CARGO_TARGET_DIR", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER",
                 "RUSTC_WORKSPACE_WRAPPER", "LD_LIBRARY_PATH", "LD_PRELOAD", "LIBRARY_PATH",
                 "CPATH", "C_INCLUDE_PATH", "CPLUS_INCLUDE_PATH", "PYTHONPATH"):
        environment.pop(name, None)
    environment["SLANGC"] = "/no-shader-compiler-required"
    cargo = os.getenv("CARGO", "cargo")
    app = bundle / "examples/foundation_rust"
    target = app / "target"
    require(not target.exists(), "target must be fresh")

    def run(command, label, *, env=None, success=True, marker=None, timeout=300):
        p = subprocess.run(command, cwd=bundle, env=env or environment, capture_output=True,
                           text=True, timeout=timeout)
        (temporary / (label + ".log")).write_text(p.stdout + p.stderr)
        if not success:
            require(p.returncode != 0, f"{label}: missing dependency unexpectedly succeeded")
        else:
            require(p.returncode == 0, f"{label}: failed; see {temporary / (label + '.log')}")
            require(not any(word in p.stdout + p.stderr for word in ("Validation Error", "VUID-", "SYNC-HAZARD")),
                    f"{label}: native validation error")
        if marker: require(marker in p.stdout + p.stderr, f"{label}: missing {marker}")
        print(f"{label}: PASS", flush=True)
        return p.stdout

    try:
        common = ["--locked", "--offline", "--manifest-path", str(app / "Cargo.toml")]
        metadata = json.loads(run([cargo, "metadata", *common, "--format-version", "1",
                                   "--filter-platform", "x86_64-unknown-linux-gnu"], "metadata"))
        require(Path(metadata["target_directory"]) == target, "ambient Cargo configuration changed target")
        local = [p for p in metadata["packages"] if p["source"] is None]
        require({p["name"] for p in local} == {"ogpu", "ogpu-vulkan-sys", "ogpu-native-rust-consumer"}, "unexpected local package graph")
        require(all(Path(p["manifest_path"]).is_relative_to(bundle) for p in local), "source dependency escaped bundle")
        runtime = next(p for p in local if p["name"] == "ogpu")
        node = next(n for n in metadata["resolve"]["nodes"] if n["id"] == runtime["id"])
        require(node["features"] == [], "legacy/default runtime features unexpectedly enabled")
        run([cargo, "build", *common, "--release", "--bins"], "build")
        # Cargo's depfiles expose include_bytes! and build.rs-generated inputs as
        # well as Rust modules. No checkout, SDK library, or old target is allowed.
        for filename in ("surface", "ogpu-native-rust-consumer"):
            text = (target / "release" / (filename + ".d")).read_text()
            dependencies = shlex.split(text.split(": ", 1)[1].split("\n\n", 1)[0].replace("\\\n", ""))
            require(dependencies and all(Path(p).resolve().is_relative_to(bundle) for p in dependencies),
                    f"{filename}: compile input escaped source bundle")
            linked = run(["ldd", str(target / "release" / filename)], "link-" + filename)
            require(not any(name in linked for name in ("libogpu", "libvulkan", "libglfw")), "unexpected static loader/C SDK dependency")
        run([cargo, "test", *common], "host-tests")
        report["build"] = True
        headless = str(target / "release/ogpu-native-rust-consumer")
        surface = str(target / "release/surface")
        run([headless], "missing-loader", env=dict(environment, OGPU_VULKAN_LIBRARY="/missing-ogpu-loader.so"), success=False, timeout=30)
        if not args.no_gpu:
            output = run([headless], "headless", marker="NATIVE_RUST_PASS", timeout=30)
            require("NATIVE_RUST_EXTENDED_PASS" in output and "NATIVE_RUST_SYNC_PASS" in output, "missing headless phases")
            report["runs"].append("headless")
        if args.presentation:
            # Discovery happens before GLFW loading. This negative needs a
            # presentation-capable adapter, unlike the no-GPU missing-loader check.
            run([surface, "--scene"], "missing-glfw", env=dict(environment, OGPU_GLFW_LIBRARY="/missing-ogpu-glfw.so"),
                success=False, marker="load GLFW 3.4", timeout=30)
            for option, marker in ((None, "SURFACE_SETUP_PASS"), ("--present", "PRESENTATION_PASS"),
                                   ("--scene-check", "SCENE_PASS"), ("--scene-smoke", "SCENE_PASS")):
                label = option.removeprefix("--") if option else "surface-setup"
                run([surface, *([option] if option else [])], label, marker=marker, timeout=30)
                report["runs"].append(label)
        report["complete"] = True
    except Exception as error:
        report["error"] = str(error)
        raise
    finally:
        (temporary / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print("Native Rust isolated build/selected checks PASS; human interaction remains unverified.")


if __name__ == "__main__": main()
