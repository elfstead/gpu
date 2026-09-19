#!/usr/bin/env python3
"""Small-compute native strategy frontier; not an application benchmark."""
import argparse
import json
import math
import os
from pathlib import Path
import shlex
import statistics
import subprocess
import sys
import tempfile
sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / "examples/learned_image"))
# Avoid the learned-image module named run: this script has a distinct import name
# under the tests, and the helper's bare import resolves through the inserted path.
import benchmark as bench
import compare_native

POLICIES = ("fresh", "reset", "replay", "ogpu")
BUILD = ROOT / "target/performance-frontier"


def require(value, message):
    if not value:
        raise RuntimeError(message)


def digest(path):
    import hashlib
    return hashlib.sha256(path.read_bytes()).hexdigest()


def order(round_index):
    return POLICIES[round_index % 4:] + POLICIES[:round_index % 4]


def parse(stdout, policy, slots, dispatches, count, validate):
    rows = bench.records(stdout, "RESULT ")
    require(len(rows) == 1, "missing/duplicate result")
    result = rows[0]
    expected = dict(policy=policy, slots=slots, dispatches=dispatches, frames=count,
                    warmups=0 if validate else 100, validation=validate, requested_bytes=slots * 388)
    require(all(result.get(k) == v for k, v in expected.items()), "unexpected execution policy")
    require(all(bench.finite(result[k]) and result[k] > 0 for k in ("setup_ms", "wall_ms")), "invalid clocks")
    require("Frontier full-output/guards PASS; all slots drained" in stdout, "missing correctness/drain marker")
    identity = bench.records(stdout, "DEVICE ")
    require(len(identity) == 1, "missing/duplicate device identity")
    frames = bench.records(stdout, "FRAME ")
    require(len(frames) == (0 if validate else count), "incomplete/extra samples")
    for index, frame in enumerate(frames):
        require(set(frame) == {"index", "record_ms", "submit_ms", "wait_ms", "latency_ms"}
                and frame["index"] == index, "invalid sample fields/order")
        require(all(bench.finite(frame[k]) and frame[k] >= 0 for k in frame if k != "index"), "invalid sample clock")
        require(frame["latency_ms"] + 1e-6 >= frame["record_ms"] + frame["submit_ms"] + frame["wait_ms"], "inconsistent clocks")
    return result, identity[0], frames


def summary(frames):
    fields = ("record_ms", "submit_ms", "wait_ms", "latency_ms", "record_submit_ms")
    result = {}
    for key in fields:
        values = sorted(f["record_ms"] + f["submit_ms"] if key == "record_submit_ms" else f[key] for f in frames)
        result[key] = dict(mean=statistics.mean(values), median=statistics.median(values),
                           p95=values[math.ceil(len(values) * .95) - 1], minimum=values[0], maximum=values[-1])
    return result


def build(environment):
    BUILD.mkdir(parents=True, exist_ok=True)
    subprocess.run(["cargo", "build", "--release", "-p", "ogpu"], cwd=ROOT, env=environment, check=True)
    cc = shlex.split(environment.get("CC", "cc"))
    flags = ["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-Iinclude",
             "-Ivendor/Vulkan-Headers/include", "-Iexamples/learned_image/generated"]
    for name, source, extra in (("native", "small.c", ["-DFRONTIER_NATIVE", "-ldl"]),
                                ("ogpu", "small.c", ["-Ltarget/release", "-logpu"]),
                                ("test-small", "test_small.c", ["-ldl"])):
        subprocess.run([*cc, *flags, str(HERE / source), *extra, "-o", str(BUILD / name)], cwd=ROOT, env=environment, check=True)
    symbols = subprocess.check_output(["nm", "-u", str(BUILD / "native")], text=True)
    require("ogpu" not in symbols.lower(), "native control links OGPU")
    subprocess.run([*cc, *flags, "-fPIC", "-shared", "-Wl,-Bsymbolic",
                    "examples/learned_image/trace_memory.c", "-ldl", "-o", str(BUILD / "trace-memory.so")], cwd=ROOT, env=environment, check=True)
    for executable, arguments in (("native", ["--selftest"]), ("ogpu", ["--selftest"]), ("test-small", [])):
        completed = subprocess.run([str(BUILD / executable), *arguments], env=environment, capture_output=True, text=True)
        (BUILD / f"{executable}.tests.txt").write_text(completed.stdout + completed.stderr)
        require(completed.returncode == 0, f"host test failed: {executable}")
    subprocess.run([sys.executable, str(HERE / "test_runner.py")], env=environment, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--software", action="store_true", help="64-frame validated controls only; no timing")
    args = parser.parse_args()
    environment = os.environ.copy()
    environment["LD_LIBRARY_PATH"] = str(ROOT / "target/release") + ":" + environment.get("LD_LIBRARY_PATH", "")
    build(environment)
    if args.check:
        print("Frontier build and host tests PASS; no GPU"); return
    require("VK_LAYER_KHRONOS_validation" in environment.get("VK_INSTANCE_LAYERS", "")
            and environment.get("VK_LAYER_VALIDATE_SYNC") == "1" and not environment.get("VK_LOADER_LAYERS_DISABLE"), "enable synchronization validation")
    require(environment.get("VK_DRIVER_FILES") or environment.get("VK_ICD_FILENAMES"), "select one ICD")
    require(not environment.get("OGPU_TRACE_LOADER"), "start without trace loader")
    destination = Path(tempfile.mkdtemp(prefix="small-", dir=BUILD))
    print(f"Frontier artifacts: {destination}", flush=True)
    source_names = ("examples/performance_frontier/small.c", "examples/performance_frontier/run.py",
                    "examples/performance_frontier/test_small.c", "examples/performance_frontier/test_runner.py",
                    "examples/performance_frontier/export.py",
                    "examples/compiler/transform.generated.h", "examples/compiler/transform.slang",
                    "examples/learned_image/native.c", "examples/learned_image/native_workload.h", "examples/learned_image/extent.h",
                    "include/ogpu.h", "vendor/Vulkan-Headers/include/vulkan/vulkan_core.h")
    report = dict(schema=1, scope="small dependent compute; strategy comparison, not isolated API overhead",
                  revision=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                  dirty=bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True)),
                  sources={name: digest(ROOT / name) for name in source_names},
                  artifacts={name: digest(BUILD / name) for name in ("native", "ogpu", "trace-memory.so")},
                  library_sha256=digest(ROOT / "target/release/libogpu.so"),
                  environment={k: environment.get(k) for k in ("VK_DRIVER_FILES", "VK_ICD_FILENAMES", "OGPU_VULKAN_LIBRARY", "VK_LAYER_PATH")},
                  software=args.software, validation=[], timing=[], memory=[], complete=False)
    for path in sorted((ROOT / "examples/learned_image/generated").glob("*.h")):
        report["sources"][str(path.relative_to(ROOT))] = digest(path)
    def save():
        (destination / "report.json").write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    save()
    identity = None
    def invoke(policy, slots, dispatches, count, validate, env, label):
        nonlocal identity
        output = destination / label; output.mkdir()
        binary = "ogpu" if policy == "ogpu" else "native"
        completed = subprocess.run([str(BUILD / binary), policy, str(slots), str(dispatches), str(count),
                                    "validate" if validate else "measure"], env=env, capture_output=True, text=True)
        (output / "stdout.txt").write_text(completed.stdout); (output / "stderr.txt").write_text(completed.stderr)
        require(completed.returncode == 0 and "Validation Error:" not in completed.stdout + completed.stderr, f"control failed: {output}")
        result, device, frames = parse(completed.stdout, policy, slots, dispatches, count, validate)
        if identity is None:
            identity = device; report["identity"] = identity
        require(device == identity, "device mismatch")
        return dict(label=label, result=result, frames=frames, device=device,
                    queues=bench.records(completed.stdout, "QUEUE "),
                    device_description=[line for line in completed.stdout.splitlines() if line.startswith("Native device:")],
                    stdout_sha256=digest(output / "stdout.txt"), stderr_sha256=digest(output / "stderr.txt")), completed.stderr
    traced = environment.copy()
    traced["OGPU_TRACE_LOADER"] = environment.get("OGPU_VULKAN_LIBRARY", "libvulkan.so.1")
    traced["OGPU_VULKAN_LIBRARY"] = str(BUILD / "trace-memory.so")
    signatures = {}
    for slots in (1, 3):
        for dispatches in (1, 64):
            for policy in POLICIES:
                row, stderr = invoke(policy, slots, dispatches, 64 if args.software else 1000, True, traced,
                                     f"validation-{slots}-{dispatches}-{policy}")
                memory = bench.parse_memory(stderr)
                require(memory["allocations"] == memory["frees"] == memory["peak_count"] == slots, "allocation growth/leak")
                signature = [(r["bytes"], r["type"]) for r in bench.records(stderr, "ALLOCATE ")]
                require(signature == signatures.setdefault(slots, signature), "allocation policy mismatch")
                row["memory"] = memory
                row["trace"] = [line for line in stderr.splitlines() if line.startswith(("ALLOCATE ", "FREE ", "MEMORY_SUMMARY "))]
                report["validation"].append(row); save()
                print(f"Validated {slots} slots / {dispatches} dispatches / {policy}: full outputs, guards, frees PASS", flush=True)
    if not args.software:
        timing = compare_native.timing_environment(environment)
        for round_index in range(3):
            for slot_index, slots in enumerate((1, 3)):
                for dispatch_index, dispatches in enumerate((1, 64)):
                    for policy in order(round_index + slot_index * 2 + dispatch_index):
                        row, _ = invoke(policy, slots, dispatches, 1000, False, timing,
                                        f"timing-{round_index}-{slots}-{dispatches}-{policy}")
                        row["round"] = round_index; row["statistics"] = summary(row["frames"])
                        report["timing"].append(row); save()
                        print(f"Timed r{round_index} {slots}/{dispatches} {policy}: {row['result']['wall_ms']:.3f} ms / 1000 frames", flush=True)
        # Separate timing-mode traces: same warmup/final-only validation, no traced timing claim.
        for slots in (1, 3):
            for dispatches in (1, 64):
                for policy in POLICIES:
                    row, stderr = invoke(policy, slots, dispatches, 1000, False, traced,
                                         f"memory-{slots}-{dispatches}-{policy}")
                    row.pop("frames")
                    memory = bench.parse_memory(stderr)
                    require(memory["allocations"] == memory["frees"] == memory["peak_count"] == slots, "timed allocation growth/leak")
                    require([(a["bytes"], a["type"]) for a in bench.records(stderr, "ALLOCATE ")] == signatures[slots], "timed memory mismatch")
                    row["memory"] = memory
                    row["trace"] = [line for line in stderr.splitlines() if line.startswith(("ALLOCATE ", "FREE ", "MEMORY_SUMMARY "))]
                    report["memory"].append(row); save()
    report["complete"] = True; save()
    print(f"Frontier small-compute PASS: {destination}")


if __name__ == "__main__":
    main()
