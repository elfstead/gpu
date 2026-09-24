#!/usr/bin/env python3
"""P4 global/range/split dependency controls; no new public runtime surface."""
import argparse
import csv
import importlib.util
import itertools
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
ORDERS = ("a-bc", "ab-c", "ba-c", "a-cb")
POLICIES = tuple((s, o) for s in ("global", "buffer", "ogpu") for o in ORDERS) + (("split", "a-bc"),)
MATRIX = tuple((k, s, o) for k in (64, 4096) for s, o in POLICIES)
FIELDS = ("submit_ms", "wait_ms", "latency_ms")


def barrier_edges(order):
    before, after = order.split("-")
    return {(a, b) for a in before for b in after}


def graph_check():
    # Exhaustive cuts and orders, not a heuristic search or a GPU timing model.
    candidates = {"".join(p[:cut])+"-"+"".join(p[cut:])
                  for p in itertools.permutations("abc") for cut in (1, 2)
                  if p.index("a") < cut <= p.index("c")}
    f.require(candidates == set(ORDERS), "order enumeration incomplete")
    for order in candidates:
        edges = barrier_edges(order)
        f.require(("a", "c") in edges and len(edges) == 2, "missing/extra graph edges")
    return {o: sorted("".join(e) for e in barrier_edges(o)) for o in ORDERS}


def parse(stdout, key, count, validate):
    kib, strategy, order = key
    rows = f.bench.records(stdout, "DEPENDENCY_RESULT ")
    identity = f.bench.records(stdout, "DEVICE ")
    f.require(len(rows) == len(identity) == 1, "missing/duplicate identity or result")
    r = rows[0]
    wanted = dict(kib=kib, strategy=strategy, order=order, frames=count,
                  warmups=0 if validate else 100, validation=validate,
                  event_count=int(strategy == "split"))
    f.require(all(r.get(k) == v for k, v in wanted.items()), "wrong dependency policy")
    f.require(all(f.bench.finite(r[k]) and r[k] > 0 for k in ("wall_ms", "setup_ms")), "invalid clocks")
    f.require("P4 exact X/Y outputs and guards PASS; all executions drained" in stdout, "missing oracle/drain")
    frames = f.bench.records(stdout, "FRAME ")
    f.require(len(frames) == (0 if validate else count), "wrong sample count")
    for i, frame in enumerate(frames):
        f.require(set(frame) == {"index", *FIELDS} and frame["index"] == i, "wrong sample fields/order")
        f.require(all(f.bench.finite(frame[k]) and frame[k] >= 0 for k in FIELDS), "invalid sample")
        f.require(abs(frame["submit_ms"] + frame["wait_ms"] - frame["latency_ms"]) < 1e-6, "clock mismatch")
    return r, identity[0], frames


def summarize(frames):
    result = {}
    for k in FIELDS:
        v = sorted(row[k] for row in frames)
        result[k] = dict(mean=statistics.mean(v), median=statistics.median(v),
                         p95=v[math.ceil(len(v)*.95)-1], minimum=v[0], maximum=v[-1])
    return result


def check_memory(row, stdout, stderr, signatures):
    memory = f.bench.parse_memory(stderr)
    f.require(memory["allocations"] == memory["frees"] == memory["peak_count"] == 2, "allocation leak/growth")
    allocations = f.bench.records(stderr, "ALLOCATE ")
    signature = [(a["bytes"], a["type"]) for a in allocations]
    kib = row["result"]["kib"]
    sizes = [64*1024+128, kib*1024+128]
    f.require(len(signature) == 2 and all(a[0] >= size for a, size in zip(signature, sizes)), "undersized backing")
    f.require(memory["peak_bytes"] == sum(a[0] for a in signature), "memory total mismatch")
    f.require(signature == signatures.setdefault(kib, signature), "allocation policy mismatch")
    native = f.bench.records(stdout, "NATIVE_BUFFER ")
    if row["result"]["strategy"] != "ogpu":
        f.require(len(native) == 2 and all(b["host"] is True and b["requested"] == size for b, size in zip(native, sizes)), "native placement/size mismatch")
        f.require(signature == [(b["allocated"], b["type"]) for b in native], "native backing mismatch")
    row["memory"] = memory
    row["trace"] = [x for x in stderr.splitlines() if x.startswith(("ALLOCATE ", "FREE ", "MEMORY_SUMMARY "))]
    row["native_buffers"] = native


def build(env):
    f.build(env)
    cc = shlex.split(env.get("CC", "cc"))
    flags = ["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-Iinclude",
             "-Ivendor/Vulkan-Headers/include", "-Iexamples/learned_image/generated"]
    for suffix, extra in (("native", ["-DFRONTIER_NATIVE", "-ldl"]), ("ogpu", ["-Ltarget/release", "-logpu"])):
        binary = f.BUILD / ("dependencies-"+suffix)
        subprocess.run([*cc, *flags, str(f.HERE / "dependencies.c"), *extra, "-o", str(binary)], cwd=f.ROOT, env=env, check=True)
        subprocess.run([str(binary), "--selftest"], env=env, check=True)
    f.require("ogpu" not in subprocess.check_output(["nm", "-u", str(f.BUILD / "dependencies-native")], text=True).lower(), "native links OGPU")
    test = f.BUILD/"test-dependencies"
    subprocess.run([*cc, *flags, str(f.HERE/"test_dependencies.c"), "-ldl", "-o", str(test)], cwd=f.ROOT, env=env, check=True)
    subprocess.run([str(test)], env=env, check=True)
    subprocess.run([sys.executable, str(f.HERE / "test_dependencies.py")], env=env, check=True)


def source_hashes():
    files = [f.HERE / n for n in ("dependencies.c", "dependencies.py", "test_dependencies.c", "test_dependencies.py", "small.c", "run.py")]
    files += [f.ROOT / n for n in ("include/ogpu.h", "Cargo.lock", "examples/compiler/transform.generated.h",
        "examples/learned_image/native.c", "examples/learned_image/native_workload.h", "examples/learned_image/extent.h",
        "examples/learned_image/trace_memory.c", "examples/learned_image/allocation_tracker.h",
        "examples/learned_image/benchmark.py", "examples/learned_image/compare_native.py", "examples/learned_image/run.py",
        "vendor/Vulkan-Headers/include/vulkan/vulkan_core.h")]
    files += sorted((f.ROOT / "crates/ogpu/src").glob("*.rs"))
    files += sorted((f.ROOT / "examples/learned_image/generated").glob("*.h"))
    return {str(p.relative_to(f.ROOT)): f.digest(p) for p in files}


def export(source, destination):
    r = json.loads(source.read_text()); f.require(r["schema"] == 1 and r["complete"], "incomplete report")
    f.require(r["graph"] == graph_check(), "graph mismatch")
    signatures = {}; samples = []
    for section in ("validation", "timing", "memory"):
        expected = set() if r["software"] and section != "validation" else (
            {(*key, i) for key in MATRIX for i in range(3)} if section == "timing" else set(MATRIX))
        keys = []
        for row in r[section]:
            key = tuple(row["result"][k] for k in ("kib", "strategy", "order"))
            keys.append((*key, row["round"]) if section == "timing" else key)
            folder = source.parent / row["label"]
            for name in ("stdout", "stderr"):
                f.require(f.digest(folder / (name+".txt")) == row[name+"_sha256"], "log hash mismatch")
            stdout = (folder/"stdout.txt").read_text(); stderr = (folder/"stderr.txt").read_text()
            f.require("Validation Error:" not in stdout+stderr, "validation error")
            count = (32 if r["software"] else 256) if section == "validation" else 1000
            result, identity, frames = parse(stdout, key, count, section == "validation")
            f.require(result == row["result"] and identity == r["identity"], "result/device mismatch")
            if section == "timing":
                f.require(frames == row["frames"] and summarize(frames) == row["statistics"], "sample/statistics mismatch")
                samples.extend(dict(kib=key[0], strategy=key[1], order=key[2], round=row["round"], **x) for x in frames)
                del row["frames"]
            else:
                checked = dict(result=result); check_memory(checked, stdout, stderr, signatures)
                f.require(all(checked[k] == row[k] for k in ("memory", "trace", "native_buffers")), "trace mismatch")
        f.require(len(keys) == len(expected) and set(keys) == expected, "matrix mismatch")
    f.require(source_hashes() == r["sources"], "sources changed")
    destination.mkdir(exist_ok=False)
    if samples:
        with (destination/"samples.csv").open("w", newline="") as out:
            w = csv.DictWriter(out, fieldnames=list(samples[0]), lineterminator="\n"); w.writeheader(); w.writerows(samples)
        r["raw_samples"] = "samples.csv"; r["raw_samples_sha256"] = f.digest(destination/"samples.csv")
    r["input_report_sha256"] = f.digest(source)
    r["export_scope"] = "graph, complete matrices, logs, samples/statistics, sources, device and allocation signatures rechecked"
    (destination/"report.json").write_text(json.dumps(r, indent=2, allow_nan=False)+"\n")
    print(f"Dependency export PASS: {destination}, {len(samples)} samples")


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--check", action="store_true"); p.add_argument("--software", action="store_true")
    p.add_argument("--export", nargs=2, type=Path)
    args = p.parse_args()
    if args.export: export(*args.export); return
    env = os.environ.copy(); env["LD_LIBRARY_PATH"] = str(f.ROOT/"target/release")+":"+env.get("LD_LIBRARY_PATH", "")
    build(env)
    if args.check: print("Dependency build/host checks PASS"); return
    f.require("VK_LAYER_KHRONOS_validation" in env.get("VK_INSTANCE_LAYERS", "") and env.get("VK_LAYER_VALIDATE_SYNC") == "1"
              and not env.get("VK_LOADER_LAYERS_DISABLE"), "enable sync validation")
    f.require((env.get("VK_DRIVER_FILES") or env.get("VK_ICD_FILENAMES")) and not env.get("OGPU_TRACE_LOADER"), "select one ICD without tracing")
    dest = Path(tempfile.mkdtemp(prefix="dependencies-", dir=f.BUILD)); print(f"Dependency artifacts: {dest}", flush=True)
    r = dict(schema=1, scope="P4 A->C with independent B; one queue/slot, two HOST allocations",
             revision=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=f.ROOT, text=True).strip(),
             dirty=bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=f.ROOT, text=True)),
             software=args.software, graph=graph_check(), sources=source_hashes(),
             artifacts={n: f.digest(f.BUILD/n) for n in ("dependencies-native", "dependencies-ogpu", "trace-memory.so")},
             library_sha256=f.digest(f.ROOT/"target/release/libogpu.so"),
             environment={k: env.get(k) for k in ("VK_DRIVER_FILES", "VK_ICD_FILENAMES", "OGPU_VULKAN_LIBRARY", "VK_LAYER_PATH")},
             validation=[], timing=[], memory=[], complete=False)
    def save(): (dest/"report.json").write_text(json.dumps(r, indent=2, allow_nan=False)+"\n")
    save(); signatures = {}
    traced = env.copy(); traced["OGPU_TRACE_LOADER"] = env.get("OGPU_VULKAN_LIBRARY", "libvulkan.so.1")
    traced["OGPU_VULKAN_LIBRARY"] = str(f.BUILD/"trace-memory.so")
    def invoke(key, count, mode, environment, label):
        kib, strategy, order = key; folder = dest/label; folder.mkdir()
        binary = f.BUILD/("dependencies-ogpu" if strategy == "ogpu" else "dependencies-native")
        call = subprocess.run([str(binary), strategy, order, str(kib), str(count), mode], env=environment,
                              capture_output=True, text=True, timeout=120)
        (folder/"stdout.txt").write_text(call.stdout); (folder/"stderr.txt").write_text(call.stderr)
        f.require(call.returncode == 0 and "Validation Error:" not in call.stdout+call.stderr, f"control failed: {folder}")
        result, identity, frames = parse(call.stdout, key, count, mode == "validate")
        f.require(identity == r.setdefault("identity", identity), "device mismatch")
        row = dict(label=label, result=result, stdout_sha256=f.digest(folder/"stdout.txt"), stderr_sha256=f.digest(folder/"stderr.txt"))
        if environment.get("OGPU_TRACE_LOADER"): check_memory(row, call.stdout, call.stderr, signatures)
        else: row.update(frames=frames, statistics=summarize(frames))
        return row
    for key in MATRIX:
        r["validation"].append(invoke(key, 32 if args.software else 256, "validate", traced, "validation-"+"-".join(map(str,key)))); save()
        print(f"Validated {key}", flush=True)
    if not args.software:
        timing = f.compare_native.timing_environment(env)
        for round_index in range(3):
            for kib in (64,4096):
                for strategy, order in f.order(round_index, POLICIES):
                    key = (kib,strategy,order)
                    row = invoke(key,1000,"measure",timing,f"timing-{round_index}-{kib}-{strategy}-{order}")
                    row["round"] = round_index; r["timing"].append(row); save()
                    print(f"Timed r{round_index} {key}: {row['result']['wall_ms']:.3f} ms", flush=True)
        for key in MATRIX:
            r["memory"].append(invoke(key,1000,"measure",traced,"memory-"+"-".join(map(str,key)))); save()
    r["complete"] = True; save(); print(f"Dependency PASS: {dest}")


if __name__ == "__main__": main()
