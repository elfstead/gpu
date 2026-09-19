#!/usr/bin/env python3
"""Build/test direct Vulkan transfers or the generated workload."""
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


def allocation_evidence(stdout, stderr, metadata, engine):
    memory = bench.parse_memory(stderr)
    allocations = bench.records(stderr, "ALLOCATE ")
    signature = [{"bytes":a["bytes"], "type":a["type"]} for a in allocations]
    expected_count = 10 if metadata["mode"] == "resident" else 9
    run.require(memory["allocations"] == memory["peak_count"] == expected_count, "unexpected allocation count")
    buffers, images = bench.records(stdout, "NATIVE_BUFFER "), bench.records(stdout, "NATIVE_IMAGE ")
    if engine == "native":
        run.require(len(buffers) == expected_count - 1 and len(images) == 1, "missing native allocation descriptions")
        run.require(sum(b["requested"] for b in buffers if b["host"]) == metadata["host_buffers"]
                    and sum(b["requested"] for b in buffers if not b["host"]) == metadata["device_buffers"]
                    and images[0]["logical"] == metadata["image_logical"], "native requested bytes mismatch")
        run.require(all(b["allocated"] >= b["requested"] and (b["flags"] & (2 if b["host"] else 1)) for b in buffers),
                    "native buffer placement mismatch")
        # Allocation order is five DEVICE buffers, upload/readback/draw, image,
        # then optional resident input B, identically in both implementations.
        ordered = [*buffers[:8], images[0], *buffers[8:]]
        run.require(signature == [{"bytes":a["allocated"], "type":a["type"]} for a in ordered],
                    "native descriptions differ from independent trace")
    return dict(summary=memory, signature=signature, buffers=buffers, images=images,
                trace=[line for line in stderr.splitlines() if line.startswith(("ALLOCATE ", "FREE ", "MEMORY_SUMMARY "))])


def workload(build, environment, scale):
    """Full CPU gate once per extent; exact equality covers each mode/engine/layout."""
    destination = Path(tempfile.mkdtemp(prefix="workload-", dir=build))
    print(f"Native workload artifacts: {destination}", flush=True)
    sources = ("native.c", "native_workload.h", "test_native.c", "run_native.py", "test_native_runner.py", "extent.h", "app.c",
               "trace_memory.c", "allocation_tracker.h")
    report = dict(scope="validated native/OGPU output equality and matched timestamp policy; no performance samples",
                  revision=subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
                  dirty=bool(subprocess.check_output(["git", "status", "--porcelain"], text=True)),
                  source_sha256={name: run.digest_file(run.HERE / name) for name in sources},
                  model_sha256=run.digest_file(run.HERE / "model.json"),
                  artifacts={}, environment={k: environment.get(k) for k in
                                            ("VK_DRIVER_FILES", "VK_ICD_FILENAMES", "OGPU_VULKAN_LIBRARY", "VK_LAYER_PATH")},
                  runs=[])
    for variant in ("original", "mutated"):
        for engine in ("native", "ogpu"):
            executable = build / f"native-{variant}" if engine == "native" else build.parent / f"app-{variant}"
            report["artifacts"][f"{engine}-{variant}"] = run.digest_file(executable)
        headers = run.HERE / "generated" if variant == "original" else build.parent / "compiler/mutated/generated"
        for header in sorted(headers.glob("*.h")):
            report["artifacts"][f"{variant}/{header.name}"] = run.digest_file(header)
    report["artifacts"]["libogpu.so"] = run.digest_file(run.ROOT / "target/release/libogpu.so")
    report["artifacts"]["trace-memory.so"] = run.digest_file(build / "trace-memory.so")
    report["artifacts"]["vulkan_core.h"] = run.digest_file(run.ROOT / "vendor/Vulkan-Headers/include/vulkan/vulkan_core.h")
    traced = environment.copy()
    traced["OGPU_TRACE_LOADER"] = environment.get("OGPU_VULKAN_LIBRARY", "libvulkan.so.1")
    traced["OGPU_VULKAN_LIBRARY"] = str(build / "trace-memory.so")
    def save():
        (destination / "report.json").write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    save()
    identity_baseline = None
    for extent in ((65, 47, 131, 95), *(bench.EXTENTS if scale else ())):
        small = extent[0] == 65
        reference = build.parent / ("reference" if small else "reference-scale")
        manifest = json.loads((reference / "manifest.json").read_text())
        cases = [c for c in manifest["cases"] if tuple(c["input_size"] + c["output_size"]) == extent
                 and c["seed"] in (2001, 2002)][:2]
        run.require(len(cases) == 2 and [c["seed"] for c in cases] == [2001, 2002], "missing A/B fixtures")
        run.verified(run.HERE / "model.json", manifest["model_sha256"])
        run.verified(reference / "weights.f32", manifest["weights_sha256"])
        for case in cases:
            for name, digest in case["sha256"].items():
                run.verified(reference / case["name"] / name, digest)
        baseline = None
        memory_baselines = {}
        label = "x".join(str(v) for v in extent)
        for variant in (("original", "mutated") if small else ("original",)):
            for engine in ("native", "ogpu"):
                executable = build / f"native-{variant}" if engine == "native" else build.parent / f"app-{variant}"
                for mode in ("end-to-end", "resident"):
                    output = destination / label / f"{engine}-{variant}-{mode}"
                    stdout, stderr = bench.invoke(executable, "--validate", mode, extent, reference, cases, output, traced)
                    metadata = bench.records(stdout, "MEASUREMENT ")
                    run.require(len(metadata) == 1, "missing/duplicate metadata")
                    bench.check_metadata(metadata[0], extent, mode, True)
                    identity = identity_evidence(stdout)
                    if identity_baseline is None:
                        identity_baseline = identity
                    else:
                        run.require(identity == identity_baseline, "device or timestamp clock mismatch")
                    memory = allocation_evidence(stdout, stderr, metadata[0], engine)
                    if mode not in memory_baselines:
                        memory_baselines[mode] = memory["signature"]
                    else:
                        run.require(memory["signature"] == memory_baselines[mode], "native/OGPU allocation size/type mismatch")
                    if baseline is None:
                        errors = bench.check_validation(output, reference, cases)
                        baseline = output
                    else:
                        for name in bench.output_files():
                            run.require(run.same_file(output / name, baseline / name), f"native/OGPU/mode/layout mismatch: {output / name}")
                    report["runs"].append(dict(extent=extent, engine=engine, variant=variant, mode=mode,
                                               metadata=metadata[0], errors=errors, memory=memory, identity=identity,
                                               device_description=[line for line in stdout.splitlines() if line.startswith(("Native device:", "Learned-image device:"))],
                                               output_sha256={name: run.digest_file(output / name) for name in bench.output_files()}))
                    save()
                    print(f"Validated {label} {engine}/{variant}/{mode}: all intermediates/final, guards, A/B/A PASS", flush=True)
    report["complete"] = True
    save()
    print(f"Native/OGPU workload correctness PASS: {destination}; no performance comparison")
    return destination, report


def identity_evidence(stdout):
    device, clock = bench.records(stdout, "DEVICE "), bench.records(stdout, "CLOCK ")
    run.require(len(device) == len(clock) == 1, "missing/duplicate device/clock identity")
    c = clock[0]
    run.require(type(c["supported"]) is bool and type(c["bits"]) is int and bench.finite(c["period_ns"]), "invalid clock identity")
    run.require((c["supported"] and 36 <= c["bits"] <= 64 and c["period_ns"] > 0)
                or (not c["supported"] and c["bits"] == 0 and c["period_ns"] == 0), "invalid clock support")
    return dict(device=device[0], clock=c)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="build and injected host tests only")
    parser.add_argument("--workload", action="store_true", help="regenerate interfaces/fixtures and validate native/OGPU workload")
    parser.add_argument("--scale", action="store_true", help="also validate four large groups (implies --workload)")
    args = parser.parse_args()
    os.chdir(run.ROOT)
    build = run.ROOT / "target/learned-image/native-control"
    build.mkdir(parents=True, exist_ok=True)
    if args.workload or args.scale:
        command = [sys.executable, str(run.HERE / "run.py"), "--check"]
        if args.scale:
            command.append("--scale")
        run.command(*command)
    cc = shlex.split(os.getenv("CC", "cc"))
    flags = ["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-Ivendor/Vulkan-Headers/include", "-Iinclude"]
    for source, name in (("native.c", "native"), ("test_native.c", "test-native")):
        run.command(*cc, *flags, f"-I{run.HERE / 'generated'}", str(run.HERE / source), "-ldl", "-o", str(build / name))
    if args.workload or args.scale:
        for variant in ("original", "mutated"):
            headers = run.HERE / "generated" if variant == "original" else build.parent / "compiler/mutated/generated"
            run.command(*cc, *flags, f"-I{headers}", str(run.HERE / "native.c"), "-ldl", "-o", str(build / f"native-{variant}"))
    tests = subprocess.run([str(build / "test-native")], capture_output=True, text=True)
    (build / "tests.stdout.txt").write_text(tests.stdout)
    (build / "tests.stderr.txt").write_text(tests.stderr)
    run.require(tests.returncode == 0, f"native host tests failed; see {build}")
    print("Native host policy/range/failure-cleanup tests PASS (expected failure diagnostics retained)", flush=True)
    run.command(sys.executable, str(run.HERE / "test_native_runner.py"))
    # Inspect the actual ELF, not just the link command: no public or private
    # OGPU runtime symbol is an allowed dependency of this control.
    for name in (("native", "native-original", "native-mutated") if args.workload or args.scale else ("native",)):
        symbols = subprocess.check_output(["nm", "-u", str(build / name)], text=True)
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
    if args.workload or args.scale:
        workload(build, environment, args.scale)
        return
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
                                 ("native.c", "native_workload.h", "test_native.c", "run_native.py", "trace_memory.c", "allocation_tracker.h")},
                  header_sha256=run.digest_file(run.ROOT / "vendor/Vulkan-Headers/include/vulkan/vulkan_core.h"),
                  executable_sha256=run.digest_file(build / "native"),
                  environment={k: os.getenv(k) for k in ("VK_DRIVER_FILES", "VK_ICD_FILENAMES", "OGPU_VULKAN_LIBRARY", "VK_LAYER_PATH")},
                  buffers=buffers, memory=memory, complete=True)
    (destination / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(result.stdout, end="")
    print(f"Native smoke validation and allocation/free checks PASS: {destination}")


if __name__ == "__main__":
    main()
