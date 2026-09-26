#!/usr/bin/env python3
"""M3 mapped dedicated/arena stream correctness and allocation control; not timing acceptance."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
spec = importlib.util.spec_from_file_location("frontier_stream", ROOT/"examples/performance_frontier/stream.py")
stream = importlib.util.module_from_spec(spec)
spec.loader.exec_module(stream)
f = stream.f
require = f.require
BUILD = ROOT/"target/resource-reuse"


def parse(stdout, stderr, allocation, policy, slots, extent, frames):
    result, device, samples = stream.parse(stdout, policy, slots, extent, frames, True)
    require(not samples, "validation must not export timing samples")
    require("Reuse HOST padding and all slot generations retired PASS" in stdout, "missing padding/state gate")
    memory = f.bench.parse_memory(stderr)
    expected = 4+slots if allocation == "arena" else 1+8*slots
    require(memory["allocations"] == memory["frees"] == memory["peak_count"] == expected,
            "unexpected allocation count, growth or leak")
    records = f.bench.records(stdout, "REUSE_MEMORY ")
    require(len(records) == 1, "missing/duplicate allocation policy")
    record = records[0]
    require(record["allocation"] == allocation and record["buffer_allocations"] == expected-slots,
            "wrong backing strategy")
    ranges = f.bench.records(stdout, "REUSE_RANGE ")
    require(len(ranges) == slots*7, "missing range records")
    require(all(r["size"] > 0 and r["atom"] > 0 and r["offset"]%r["atom"] == 0
                and r["end"]%r["atom"] == 0 and r["offset"]+r["size"] <= r["end"] for r in ranges),
            "invalid range/atom policy")
    ends = {}
    for index, r in enumerate(ranges):
        slot, resource = divmod(index, 7)
        backing = (0 if resource < 5 else resource-4) if allocation == "arena" else index
        require((r["slot"], r["resource"], r["backing"], r["host"]) == (slot, resource, backing, resource >= 5),
                "incorrect range ownership")
        require(r["offset"] >= ends.get(backing, 0), "overlapping ranges")
        ends[backing] = r["end"]
    return dict(result=result, device=device, memory=memory, backing=record, ranges=ranges)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--preflight", action="store_true", help="12 frames instead of 1000; not long-run acceptance")
    parser.add_argument("--scale", action="store_true", help="also existing 720p/odd-video extents (Radeon)")
    args = parser.parse_args()
    require(not os.getenv("CARGO_TARGET_DIR"), "leave CARGO_TARGET_DIR unset")
    env = os.environ.copy()
    env["LD_LIBRARY_PATH"] = str(ROOT/"target/release")+":"+env.get("LD_LIBRARY_PATH", "")
    BUILD.mkdir(parents=True, exist_ok=True)
    f.build(env)
    cc = shlex.split(env.get("CC", "cc"))
    common = [*cc, "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-Iinclude",
              "-Ivendor/Vulkan-Headers/include", "-Iexamples/learned_image/generated"]
    for name, source, extra in (
        ("stream", ROOT/"examples/performance_frontier/stream.c", ["-DFRONTIER_REUSE", "-Ltarget/release", "-logpu"]),
        ("test-reuse", HERE/"test_reuse.c", ["-DNDEBUG"]),
        ("test-ranges", HERE/"test_stream_ranges.c", ["-Ltarget/release", "-logpu"]),
        ("stream-legacy", ROOT/"examples/performance_frontier/stream.c", ["-Ltarget/release", "-logpu"]),
        ("stream-native", ROOT/"examples/performance_frontier/stream.c", ["-DFRONTIER_NATIVE", "-ldl"]),
        ("test-stream", ROOT/"examples/performance_frontier/test_stream.c", ["-Ltarget/release", "-logpu"])):
        subprocess.run([*common, str(source), *extra, "-o", str(BUILD/name)], cwd=ROOT, env=env, check=True)
    for name in ("test-reuse", "test-stream", "test-ranges"):
        subprocess.run([str(BUILD/name)], env=env, check=True)
    subprocess.run([sys.executable, str(HERE/"test_runner.py")], env=env, check=True)
    if args.check:
        print("Reuse build/CPU gates PASS; no GPU"); return
    require("VK_LAYER_KHRONOS_validation" in env.get("VK_INSTANCE_LAYERS", "") and
            env.get("VK_LAYER_VALIDATE_SYNC") == "1" and not env.get("VK_LOADER_LAYERS_DISABLE"),
            "enable Vulkan/synchronization validation")
    require(env.get("VK_DRIVER_FILES") and not env.get("OGPU_TRACE_LOADER"), "select one ICD, no preinstalled tracing")
    env["OGPU_TRACE_LOADER"] = env.get("OGPU_VULKAN_LIBRARY", "libvulkan.so.1")
    env["OGPU_VULKAN_LIBRARY"] = str(f.BUILD/"trace-memory.so")
    frames = 12 if args.preflight else 1000
    extents = [(65,47,131,95)]
    if args.scale: extents += [(1280,720,2560,1440), (1919,1079,2561,1441)]
    dest = Path(tempfile.mkdtemp(prefix="stream-", dir=BUILD))
    print(f"Reuse evidence: {dest}", flush=True)
    sources = list(HERE.glob("*.h"))+list(HERE.glob("*.c"))+list(HERE.glob("*.py"))
    sources += [ROOT/"examples/performance_frontier"/n for n in ("stream.c", "small.c", "stream.py", "run.py")]
    sources += [ROOT/"include/ogpu.h", ROOT/"examples/learned_image/trace_memory.c",
                ROOT/"examples/learned_image/extent.h", ROOT/"examples/learned_image/benchmark.py"]
    sources += list((ROOT/"examples/learned_image/generated").glob("*.h"))
    report = dict(schema=1, scope="mapped dedicated/arena correctness and allocation counts; no speed claim",
                  revision=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                  dirty=bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True)),
                  frames=frames, preflight=args.preflight, complete=False,
                  sources={str(p.relative_to(ROOT)):f.digest(p) for p in sources},
                  artifacts={n:f.digest(BUILD/n) for n in ("stream", "stream-legacy", "stream-native")},
                  allocation_tracer_sha256=f.digest(f.BUILD/"trace-memory.so"),
                  runtime_sha256=f.digest(ROOT/"target/release/libogpu.so"),
                  environment={k:env.get(k) for k in ("VK_DRIVER_FILES", "VK_INSTANCE_LAYERS", "VK_LAYER_VALIDATE_SYNC", "OGPU_TRACE_LOADER")},
                  fixtures={}, runs=[])
    def save(): (dest/"report.json").write_text(json.dumps(report, indent=2, allow_nan=False)+"\n")
    save()
    identity = None
    for extent in extents:
        paths, hashes = stream.fixtures(extent)
        label = "x".join(map(str, extent)); report["fixtures"][label] = hashes
        serial = dest/(label+"-serial"); serial.mkdir()
        serial_paths = [serial/"a.rgba", serial/"b.rgba"]
        env["OGPU_STREAM_SERIAL_A"], env["OGPU_STREAM_SERIAL_B"] = map(str, serial_paths)
        requested = {}
        configurations = [("dedicated", "ogpu", 1, 12, True)]
        configurations += [(allocation, policy, slots, frames, False) for slots in (1,2,3)
                           for allocation in ("dedicated", "arena") for policy in ("ogpu", "compiled")]
        for allocation, policy, slots, count, write in configurations:
            name = f"{label}-{allocation}-{policy}-{slots}"+("-serial" if write else "")
            env["OGPU_STREAM_ALLOCATION"] = allocation
            env["OGPU_STREAM_SERIAL_MODE"] = "write" if write else "check"
            r = subprocess.run([str(BUILD/"stream"), policy, str(slots), *map(str, extent), str(count),
                                "validate", *map(str, paths)], env=env, text=True, capture_output=True)
            (dest/(name+".stdout")).write_text(r.stdout); (dest/(name+".stderr")).write_text(r.stderr)
            require(r.returncode == 0 and "Validation Error:" not in r.stdout+r.stderr, f"reuse run failed: {name}")
            row = parse(r.stdout, r.stderr, allocation, policy, slots, extent, count)
            if identity is None: identity = row["device"]
            require(row["device"] == identity, "device changed")
            backing = row["backing"]
            require(backing["buffer_requested_bytes"] == requested.setdefault(slots, backing["buffer_requested_bytes"]),
                    "dedicated/arena buffer byte budgets differ")
            row.update(label=name, serial_write=write, serial_sha256=[f.digest(p) for p in serial_paths],
                       trace=[line for line in r.stderr.splitlines() if line.startswith(("ALLOCATE ", "FREE ", "MEMORY_SUMMARY "))],
                       stdout_sha256=f.digest(dest/(name+".stdout")), stderr_sha256=f.digest(dest/(name+".stderr")))
            report["runs"].append(row); save()
            print(f"Validated {name}: {count} frames; {row['memory']['peak_count']} allocations, {row['memory']['peak_bytes']} bytes", flush=True)
    report["complete"] = True; save()
    print(f"Reuse stream PASS: {dest}")


if __name__ == "__main__": main()
