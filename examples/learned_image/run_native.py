#!/usr/bin/env python3
"""Build/test the native control foundation; no learned-image comparison yet."""
import argparse
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
sys.dont_write_bytecode = True
import benchmark as bench
import run


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="build and injected host tests only")
    args = parser.parse_args()
    os.chdir(run.ROOT)
    build = run.ROOT / "target/learned-image/native-control"
    build.mkdir(parents=True, exist_ok=True)
    cc = shlex.split(os.getenv("CC", "cc"))
    flags = ["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-Ivendor/Vulkan-Headers/include"]
    for source, name in (("native.c", "native"), ("test_native.c", "test-native")):
        run.command(*cc, *flags, str(run.HERE / source), "-ldl", "-o", str(build / name))
    tests = subprocess.run([str(build / "test-native")], capture_output=True, text=True)
    (build / "tests.stdout.txt").write_text(tests.stdout)
    (build / "tests.stderr.txt").write_text(tests.stderr)
    run.require(tests.returncode == 0, f"native host tests failed; see {build}")
    print("Native host policy/range/failure-cleanup tests PASS (expected failure diagnostics retained)", flush=True)
    # Inspect the actual ELF, not just the link command: no public or private
    # OGPU runtime symbol is an allowed dependency of this control.
    symbols = subprocess.check_output(["nm", "-u", str(build / "native")], text=True)
    run.require("ogpu" not in symbols.lower(), "native control depends on OGPU runtime")
    shim = build / "trace-memory.so"
    run.command(*cc, *flags, "-fPIC", "-shared", "-Wl,-Bsymbolic", str(run.HERE / "trace_memory.c"),
                "-ldl", "-o", str(shim))
    if args.check:
        print("Native standalone build/symbol checks PASS; no GPU execution")
        return
    environment = os.environ.copy()
    run.require("VK_LAYER_KHRONOS_validation" in environment.get("VK_INSTANCE_LAYERS", "")
                and environment.get("VK_LAYER_VALIDATE_SYNC") == "1"
                and not environment.get("VK_LOADER_LAYERS_DISABLE"), "enable synchronization validation")
    run.require(environment.get("VK_DRIVER_FILES") or environment.get("VK_ICD_FILENAMES"), "select one ICD explicitly")
    run.require(not environment.get("OGPU_TRACE_LOADER"), "start with the real Vulkan loader")
    environment["OGPU_TRACE_LOADER"] = environment.get("OGPU_VULKAN_LIBRARY", "libvulkan.so.1")
    environment["OGPU_VULKAN_LIBRARY"] = str(shim)
    destination = Path(tempfile.mkdtemp(prefix="smoke-", dir=build))
    result = subprocess.run([str(build / "native"), "--smoke"], env=environment, capture_output=True, text=True)
    (destination / "stdout.txt").write_text(result.stdout)
    (destination / "stderr.txt").write_text(result.stderr)
    run.require(result.returncode == 0 and "Validation Error:" not in result.stdout + result.stderr,
                f"native smoke failed; see {destination}")
    run.require("Native setup/address-copy A/B/A smoke PASS" in result.stdout, "missing smoke completion")
    buffers = bench.records(result.stdout, "NATIVE_BUFFER ")
    run.require(len(buffers) == 3 and [b["host"] for b in buffers] == [True, False, True]
                and all(b["requested"] == 260 and b["allocated"] >= 260 for b in buffers), "buffer policy mismatch")
    memory = bench.parse_memory(result.stderr)
    run.require(memory["allocations"] == memory["frees"] == memory["peak_count"] == 3
                and memory["peak_bytes"] == sum(b["allocated"] for b in buffers), "allocation accounting mismatch")
    report = dict(scope="native setup/address-copy smoke only; no shader execution or performance comparison",
                  revision=subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
                  dirty=bool(subprocess.check_output(["git", "status", "--porcelain"], text=True)),
                  source_sha256={name: run.digest_file(run.HERE / name) for name in
                                 ("native.c", "test_native.c", "run_native.py", "trace_memory.c", "allocation_tracker.h")},
                  header_sha256=run.digest_file(run.ROOT / "vendor/Vulkan-Headers/include/vulkan/vulkan_core.h"),
                  executable_sha256=run.digest_file(build / "native"),
                  environment={k: os.getenv(k) for k in ("VK_DRIVER_FILES", "VK_ICD_FILENAMES", "OGPU_VULKAN_LIBRARY", "VK_LAYER_PATH")},
                  buffers=buffers, memory=memory, complete=True)
    (destination / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(result.stdout, end="")
    print(f"Native smoke validation and allocation/free checks PASS: {destination}")


if __name__ == "__main__":
    main()
