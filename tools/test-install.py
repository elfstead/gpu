#!/usr/bin/env python3
"""Check a relocated installed SDK from an out-of-tree consumer directory."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def run(args, **kwargs):
    result = subprocess.run(args, text=True, capture_output=True, **kwargs)
    print(result.stdout, end="", flush=True)
    print(result.stderr, end="", file=sys.stderr, flush=True)
    if result.returncode:
        raise RuntimeError(f"command failed ({result.returncode}): {args}")
    if "Validation Error:" in result.stdout + result.stderr:
        raise RuntimeError("Vulkan validation error")
    return result.stdout


def require(value, message):
    if not value:
        raise ValueError(message)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prefix", required=True, type=Path)
    parser.add_argument("--no-gpu", action="store_true")
    parser.add_argument("--shader-check", action="store_true", help="also use installed compiler adapter (needs Slang/SPIRV-Tools)")
    args = parser.parse_args()
    installed = args.prefix.resolve()
    manifest = json.loads((installed / "share/ogpu/manifest.json").read_text())
    for relative, digest in manifest["files_sha256"].items():
        require(hashlib.sha256((installed / relative).read_bytes()).hexdigest() == digest,
                f"installed hash mismatch: {relative}")
    # Keep artifacts for inspection. No recursive deletion, source checkout copy,
    # developer environment script or reference to repository build outputs.
    temporary = Path(tempfile.mkdtemp(prefix="ogpu-external-"))
    prefix = temporary / "relocated sdk"
    shutil.copytree(installed, prefix)
    application = temporary / "independent app"
    shutil.copytree(prefix / "share/ogpu/examples/transform", application)
    environment = os.environ.copy()
    environment["PKG_CONFIG_PATH"] = str(prefix / "lib/pkgconfig")
    environment["PKG_CONFIG_LIBDIR"] = str(prefix / "lib/pkgconfig")
    environment.pop("PKG_CONFIG_SYSROOT_DIR", None)
    # Do not allow an inherited source/build path to supply ogpu or its headers.
    for key in ("LD_LIBRARY_PATH", "LD_PRELOAD", "LIBRARY_PATH", "CPATH", "C_INCLUDE_PATH", "CPLUS_INCLUDE_PATH", "PYTHONPATH"):
        environment.pop(key, None)
    environment["CARGO"] = "/no-consumer-cargo-required"
    environment["SLANGC"] = "/no-consumer-shader-compiler-required"
    print(f"Installed SDK consumer test directory: {temporary}", flush=True)
    pkg = os.getenv("PKG_CONFIG", "pkg-config")
    identity = run([pkg, "--variable=ogpu_revision", "ogpu"], cwd=application, env=environment).strip()
    require(identity == manifest["revision"], "pkg-config revision mismatch")
    abi = run([pkg, "--variable=ogpu_abi", "ogpu"], cwd=application, env=environment).strip()
    require(int(abi) == manifest["abi"], "pkg-config ABI mismatch")
    flags = run([pkg, "--cflags", "--libs", "ogpu"], cwd=application, env=environment)
    require(str(installed) not in flags, "metadata still refers to old prefix")
    run([sys.executable, "build.py"], cwd=application, env=environment)
    # These binaries are just built from our trusted source; ldd is diagnostic.
    linked = run(["ldd", "./transform"], cwd=application, env=environment)
    selected = next((line.split("=>", 1)[1].rsplit(" (", 1)[0].strip()
                     for line in linked.splitlines() if line.strip().startswith("libogpu.so =>")), None)
    require(selected is not None and Path(selected).resolve() == prefix / "lib/libogpu.so",
            "consumer did not resolve the relocated installed runtime")
    missing = environment.copy()
    missing["OGPU_VULKAN_LIBRARY"] = str(temporary / "deliberately-absent-loader.so")
    failure = subprocess.run(["./transform"], cwd=application, env=missing, text=True, capture_output=True)
    require(failure.returncode != 0 and "ogpu_probe_create" in failure.stderr,
            "absent loader must produce a visible failure")
    print("Missing loader rejects visibly PASS")
    if not args.no_gpu:
        run(["./transform"], cwd=application, env=environment)
    if args.shader_check:
        environment["SLANGC"] = os.getenv("SLANGC", "slangc")
        shader = [str(prefix / "bin/ogpu-shader"), "--source", "transform.slang", "--output",
                  "transform.generated.h", "--build-dir", "shader-build", "--name", "transform", "--stage", "compute"]
        run([*shader, "--check"], cwd=application, env=environment)
        # Use the installed tool on a changed consumer-owned shader interface.
        source = application / "transform.slang"
        original = source.read_text()
        changed = original.replace("    uint* data;\n    uint count;", "    uint count;\n    uint* data;")
        changed = changed.replace("numthreads(64, 1, 1)", "numthreads(32, 1, 1)")
        require(changed != original, "mutation fixture unchanged")
        source.write_text(changed)
        stale = subprocess.run([*shader, "--check"], cwd=application, env=environment, text=True, capture_output=True)
        require(stale.returncode != 0 and "stale generated interface" in stale.stderr,
                "modified source must fail stale-header check")
        run(shader, cwd=application, env=environment)
        run([sys.executable, "build.py"], cwd=application, env=environment)
        if not args.no_gpu:
            run(["./transform"], cwd=application, env=environment)
    print(f"External installation {'build' if args.no_gpu else 'execution'} PASS; revision={identity}, ABI={abi}; artifacts retained: {temporary}")


if __name__ == "__main__":
    main()
