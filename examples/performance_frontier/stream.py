#!/usr/bin/env python3
"""Streaming learned-image frontier, with preverified existing M1 fixtures."""
import argparse
import importlib.util
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
spec = importlib.util.spec_from_file_location("frontier", Path(__file__).with_name("run.py"))
f = importlib.util.module_from_spec(spec); spec.loader.exec_module(f)


def fixtures(extent):
    reference = f.ROOT / "target/learned-image" / ("reference" if extent[0] == 65 else "reference-scale")
    manifest = json.loads((reference / "manifest.json").read_text())
    f.require(f.digest(f.ROOT / "examples/learned_image/model.json") == manifest["model_sha256"], "model mismatch")
    cases = [next(c for c in manifest["cases"] if c["seed"] == seed and tuple(c["input_size"] + c["output_size"]) == extent) for seed in (2001, 2002)]
    paths = [reference / "weights.f32", *(reference / c["name"] / "input.f32" for c in cases),
             *(reference / c["name"] / "final.rgba" for c in cases)]
    digests = [manifest["weights_sha256"], *(c["sha256"]["input.f32"] for c in cases), *(c["sha256"]["final.rgba"] for c in cases)]
    for path, digest in zip(paths, digests):
        f.require(f.digest(path) == digest, f"fixture hash mismatch: {path}")
    return paths, dict(zip(map(str, paths), digests))


def parse(stdout, policy, slots, extent, frames, validate):
    result = f.bench.records(stdout, "STREAM "); device = f.bench.records(stdout, "DEVICE ")
    f.require(len(result) == len(device) == 1, "missing/duplicate result or identity")
    result = result[0]
    expected = dict(policy=policy, slots=slots, extent=list(extent), frames=frames,
                    warmups=0 if validate else 100, validation=validate)
    f.require(all(result[k] == v for k, v in expected.items()), "stream policy mismatch")
    f.require(result["max_rgb_delta"] in (0, 1), "pixel gate failed")
    f.require(all(f.bench.finite(result[k]) and result[k] > 0 for k in ("wall_ms", "setup_ms")), "invalid clock")
    f.require("Streaming full-frame pixels/readback guards PASS; all slots drained" in stdout, "missing correctness/drain")
    f.require("Streaming final input/weight integrity and all intermediate guards PASS" in stdout, "missing buffer integrity gate")
    samples = f.bench.records(stdout, "FRAME ")
    f.require(len(samples) == (0 if validate else frames), "sample count mismatch")
    for i, row in enumerate(samples):
        f.require(set(row) == {"index", "upload_ms", "record_ms", "submit_ms", "wait_ms", "read_ms", "latency_ms"}
                  and row["index"] == i, "bad sample order/fields")
        f.require(all(f.bench.finite(v) and v >= 0 for k, v in row.items() if k != "index"), "bad sample clock")
        f.require(row["latency_ms"] + 1e-6 >= sum(row[k] for k in ("upload_ms", "record_ms", "submit_ms", "wait_ms", "read_ms")), "inconsistent interval")
    return result, device[0], samples


def summarize(samples):
    result = f.summary(samples)
    for key in ("upload_ms", "read_ms"):
        values = sorted(row[key] for row in samples)
        result[key] = dict(mean=statistics.mean(values), median=statistics.median(values),
            p95=values[math.ceil(len(values) * .95) - 1], minimum=values[0], maximum=values[-1])
    return result


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--check", action="store_true")
    p.add_argument("--software", action="store_true", help="small odd case, 64 validated frames only")
    p.add_argument("--preflight", action="store_true", help="small odd case, 12 validated frames only")
    p.add_argument("--compiled", action="store_true", help="compiled-list correctness only; requires --software or --preflight")
    args = p.parse_args()
    f.require(not args.compiled or args.software or args.preflight, "compiled check requires a small correctness mode")
    policies = ("compiled",) if args.compiled else f.POLICIES
    env = os.environ.copy(); env["LD_LIBRARY_PATH"] = str(f.ROOT / "target/release") + ":" + env.get("LD_LIBRARY_PATH", "")
    f.build(env)
    cc = shlex.split(env.get("CC", "cc"))
    flags = ["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-Iinclude", "-Ivendor/Vulkan-Headers/include", "-Iexamples/learned_image/generated"]
    for name, extra in (("native", ["-DFRONTIER_NATIVE", "-ldl"]), ("ogpu", ["-Ltarget/release", "-logpu"])):
        subprocess.run([*cc, *flags, str(f.HERE / "stream.c"), *extra, "-o", str(f.BUILD / f"stream-{name}")], cwd=f.ROOT, env=env, check=True)
    f.require("ogpu" not in subprocess.check_output(["nm", "-u", str(f.BUILD / "stream-native")], text=True).lower(), "native links OGPU")
    subprocess.run([*cc, *flags, str(f.HERE / "test_stream.c"), "-Ltarget/release", "-logpu", "-o", str(f.BUILD / "test-stream")], cwd=f.ROOT, env=env, check=True)
    test = subprocess.run([str(f.BUILD / "test-stream")], env=env, capture_output=True, text=True)
    (f.BUILD / "test-stream.log").write_text(test.stdout + test.stderr)
    f.require(test.returncode == 0, "stream host gates failed")
    subprocess.run([sys.executable, str(f.HERE / "test_stream.py")], env=env, check=True)
    if args.check:
        print("Streaming build PASS; no GPU"); return
    f.require("VK_LAYER_KHRONOS_validation" in env.get("VK_INSTANCE_LAYERS", "")
        and env.get("VK_LAYER_VALIDATE_SYNC") == "1" and not env.get("VK_LOADER_LAYERS_DISABLE"), "enable synchronization validation")
    f.require((env.get("VK_DRIVER_FILES") or env.get("VK_ICD_FILENAMES")) and not env.get("OGPU_TRACE_LOADER"), "select one real ICD")
    small = args.software or args.preflight
    extents = ((65, 47, 131, 95),) if small else ((1280, 720, 2560, 1440), (1919, 1079, 2561, 1441))
    count = 12 if args.preflight else 64 if args.software else 1000
    selected = [(extent, *fixtures(extent)) for extent in extents]
    dest = Path(tempfile.mkdtemp(prefix="stream-", dir=f.BUILD)); print(f"Stream artifacts: {dest}", flush=True)
    sources = [f.HERE / name for name in ("small.c", "stream.c", "stream.py", "run.py", "test_stream.c", "test_stream.py", "export_stream.py")]
    sources += [f.ROOT / "examples/learned_image" / name for name in ("native.c", "native_workload.h", "extent.h", "model.json")]
    sources += sorted((f.ROOT / "examples/learned_image/generated").glob("*.h"))
    sources += [f.ROOT / "include/ogpu.h", f.ROOT / "vendor/Vulkan-Headers/include/vulkan/vulkan_core.h"]
    report = dict(schema=1, scope="streaming final-output/slot correctness and serialized single-queue strategy frontier; no intermediate reacceptance",
        revision=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=f.ROOT, text=True).strip(),
        dirty=bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=f.ROOT, text=True)),
        software=args.software, preflight=args.preflight, compiled_only=args.compiled,
        sources={str(path.relative_to(f.ROOT)): f.digest(path) for path in sources},
        artifacts={name:f.digest(f.BUILD / name) for name in ("stream-native", "stream-ogpu", "trace-memory.so")},
        library_sha256=f.digest(f.ROOT / "target/release/libogpu.so"),
        environment={k:env.get(k) for k in ("VK_DRIVER_FILES", "VK_ICD_FILENAMES", "OGPU_VULKAN_LIBRARY", "VK_LAYER_PATH")},
        fixtures={"x".join(map(str, extent)): hashes for extent, _, hashes in selected}, validation=[], timing=[], memory=[], complete=False)
    def save():
        (dest / "report.json").write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    save(); identity = None; signatures = {}
    traced = env.copy(); traced["OGPU_TRACE_LOADER"] = env.get("OGPU_VULKAN_LIBRARY", "libvulkan.so.1")
    traced["OGPU_VULKAN_LIBRARY"] = str(f.BUILD / "trace-memory.so")
    def invoke(policy, slots, extent, paths, validate, environment, label):
        nonlocal identity
        out = dest / label; out.mkdir()
        binary = f.BUILD / ("stream-ogpu" if policy in ("ogpu", "compiled") else "stream-native")
        r = subprocess.run([str(binary), policy, str(slots), *map(str, extent), str(count),
            "validate" if validate else "measure", *map(str, paths)], env=environment, capture_output=True, text=True)
        (out / "stdout.txt").write_text(r.stdout); (out / "stderr.txt").write_text(r.stderr)
        f.require(r.returncode == 0 and "Validation Error:" not in r.stdout + r.stderr, f"stream failed: {out}")
        result, device, samples = parse(r.stdout, policy, slots, extent, count, validate)
        if identity is None:
            identity = device; report["identity"] = device
        f.require(device == identity, "device mismatch")
        row = dict(label=label, result=result, samples=samples, stdout_sha256=f.digest(out / "stdout.txt"), stderr_sha256=f.digest(out / "stderr.txt"))
        if environment.get("OGPU_TRACE_LOADER"):
            memory = f.bench.parse_memory(r.stderr)
            f.require(memory["allocations"] == memory["frees"] == memory["peak_count"] == 1 + 8 * slots, "allocation growth/leak")
            signature = [(a["bytes"], a["type"]) for a in f.bench.records(r.stderr, "ALLOCATE ")]
            f.require(signature == signatures.setdefault((extent, slots), signature), "allocation policy mismatch")
            row["memory"] = memory; row["trace"] = [line for line in r.stderr.splitlines() if line.startswith(("ALLOCATE ", "FREE ", "MEMORY_SUMMARY "))]
        return row
    for extent, paths, _ in selected:
        label = "x".join(map(str, extent))
        for slots in (1, 2, 3):
            for policy in policies:
                row = invoke(policy, slots, extent, paths, True, traced, f"validation-{label}-{slots}-{policy}")
                report["validation"].append(row); save(); print(f"Validated {label} slots={slots} {policy}", flush=True)
    if not small:
        timing = f.compare_native.timing_environment(env)
        for round_index in range(3):
            for extent_index, (extent, paths, _) in enumerate(selected):
                label = "x".join(map(str, extent))
                for slots in (1, 2, 3):
                    for policy in f.order(round_index + extent_index * 3 + slots - 1):
                        row = invoke(policy, slots, extent, paths, False, timing, f"timing-{round_index}-{label}-{slots}-{policy}")
                        row["round"] = round_index; row["statistics"] = summarize(row["samples"])
                        report["timing"].append(row); save(); print(f"Timed {label} slots={slots} {policy} r{round_index}: {row['result']['wall_ms']:.3f} ms", flush=True)
        for extent, paths, _ in selected:
            label = "x".join(map(str, extent))
            for slots in (1, 2, 3):
                for policy in f.POLICIES:
                    row = invoke(policy, slots, extent, paths, False, traced, f"memory-{label}-{slots}-{policy}")
                    row.pop("samples"); report["memory"].append(row); save()
    report["complete"] = True; save(); print(f"Stream frontier PASS: {dest}")


if __name__ == '__main__':
    main()
