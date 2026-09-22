#!/usr/bin/env python3
"""P3 copy/mapping and shared-range controls; no new runtime surface."""
import argparse
import csv
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
POLICIES = ("native-copy", "mapped", "ogpu", "shared")
FIELDS = ("produce_ms", "upload_ms", "submit_ms", "wait_ms", "read_ms", "consume_ms", "latency_ms")


def matrix():
    return tuple((slots, kib, p) for slots in (1, 2) for kib in (64, 4096)
                 for p in POLICIES if slots == 2 or p != "shared")


def parse(stdout, slots, kib, policy, count, validate):
    records = f.bench.records(stdout, "HOST_RESULT "); devices = f.bench.records(stdout, "DEVICE ")
    f.require(len(records) == len(devices) == 1, "missing/duplicate host result/device")
    r = records[0]; payload = kib * 1024; span = payload + 128
    expected = dict(policy=policy, slots=slots, kib=kib, frames=count,
                    warmups=0 if validate else 100, validation=validate,
                    staging_bytes=0 if policy in ("mapped", "shared") else slots * span)
    f.require(all(r.get(k) == v for k, v in expected.items()), "host policy mismatch")
    f.require(isinstance(r["stride"], int) and r["stride"] >= span
              and (policy == "shared" or r["stride"] == span)
              and r["requested_bytes"] == slots * r["stride"], "invalid storage budget")
    n = payload // 4
    sums = [n * ((seed * 3 + 7) & 0xffffffff) + 51 * n * (n - 1) // 2 for seed in (1, 0x80001234)]
    f.require(count % (2 * slots) == 0 and r["checksum"] == count // 2 * sum(sums), "consumer checksum mismatch")
    f.require(all(f.bench.finite(r[k]) and r[k] > 0 for k in ("setup_ms", "wall_ms")), "bad host clocks")
    f.require("HOST_ACCESS full outputs/guards/checksums PASS; all slots drained" in stdout, "missing host acceptance")
    rows = f.bench.records(stdout, "HOST_FRAME ")
    f.require(len(rows) == (0 if validate else count), "sample count mismatch")
    for i, row in enumerate(rows):
        f.require(set(row) == {"index", *FIELDS} and row["index"] == i, "sample fields/order mismatch")
        f.require(all(f.bench.finite(row[k]) and row[k] >= 0 for k in FIELDS), "invalid sample clock")
        f.require(row["latency_ms"] + 1e-6 >= sum(row[k] for k in FIELDS if k != "latency_ms"), "inconsistent intervals")
    return r, devices[0], rows


def summarize(rows):
    result = {}
    for key in (*FIELDS, "cpu_access_ms"):
        values = sorted(sum(r[k] for k in ("produce_ms", "upload_ms", "read_ms", "consume_ms"))
                        if key == "cpu_access_ms" else r[key] for r in rows)
        result[key] = dict(mean=statistics.mean(values), median=statistics.median(values),
                           p95=values[math.ceil(len(values) * .95) - 1], minimum=values[0], maximum=values[-1])
    return result


def build(env):
    f.build(env)
    cc = shlex.split(env.get("CC", "cc"))
    flags = ["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-Iinclude",
             "-Ivendor/Vulkan-Headers/include", "-Iexamples/learned_image/generated"]
    for name, source, extra in (("host-native", "host_access.c", ["-DFRONTIER_NATIVE", "-ldl"]),
                                ("host-ogpu", "host_access.c", ["-Ltarget/release", "-logpu"]),
                                ("test-host-access", "test_host_access.c", ["-ldl"])):
        subprocess.run([*cc, *flags, str(f.HERE / source), *extra, "-o", str(f.BUILD / name)], cwd=f.ROOT, env=env, check=True)
        test = subprocess.run([str(f.BUILD / name), *([] if name == "test-host-access" else ["--selftest"])],
                              env=env, capture_output=True, text=True)
        (f.BUILD / f"{name}.tests.txt").write_text(test.stdout + test.stderr)
        f.require(test.returncode == 0, f"host test failed: {name}")
    f.require("ogpu" not in subprocess.check_output(["nm", "-u", str(f.BUILD / "host-native")], text=True).lower(), "native links OGPU")
    subprocess.run([sys.executable, str(f.HERE / "test_host_access.py")], env=env, check=True)


def memory_check(row, stderr):
    memory = f.bench.parse_memory(stderr); result = row["result"]
    count = 1 if result["policy"] == "shared" else result["slots"]
    f.require(memory["allocations"] == memory["frees"] == memory["peak_count"] == count, "host allocation leak/growth")
    f.require(memory["peak_bytes"] >= result["requested_bytes"], "allocation smaller than buffers")
    row["memory"] = memory
    row["trace"] = [line for line in stderr.splitlines() if line.startswith(("ALLOCATE ", "FREE ", "MEMORY_SUMMARY "))]
    return [(a["bytes"], a["type"]) for a in f.bench.records(stderr, "ALLOCATE ")]


def export(source, destination):
    report = json.loads(source.read_text())
    f.require(report.get("schema") == 1 and report.get("complete") is True, "incomplete/unknown host report")
    samples = []; signatures = {}; types = {}
    expected = set(matrix()); count = 64 if report["software"] else 1000
    for section in ("validation", "timing", "memory"):
        rows = report[section]
        wanted = set() if report["software"] and section != "validation" else (
            {(*key, r) for key in expected for r in range(3)} if section == "timing" else expected)
        keys = [(r["result"]["slots"], r["result"]["kib"], r["result"]["policy"], *([r["round"]] if section == "timing" else [])) for r in rows]
        f.require(len(keys) == len(wanted) and set(keys) == wanted, f"incomplete/duplicate {section} matrix")
        for row in rows:
            stdout, stderr = checked_logs(source, row)
            metadata_check(row, stdout)
            r = row["result"]
            result, device, frames = parse(stdout, r["slots"], r["kib"], r["policy"], count, section == "validation")
            f.require(result == r and device == report["identity"], "identity/result mismatch")
            if section == "timing":
                f.require(frames == row["frames"] and summarize(frames) == row["statistics"], "host samples/statistics mismatch")
                samples.extend(dict(slots=r["slots"], kib=r["kib"], policy=r["policy"], round=row["round"], **frame) for frame in frames)
            else:
                copy = dict(result=r); signature = memory_check(copy, stderr)
                f.require(copy["memory"] == row["memory"] and copy["trace"] == row["trace"], "host trace mismatch")
                check_signatures(signatures, types, r, signature)
            row.pop("frames", None)
    gate_keys = [(r["kib"], r["policy"]) for r in report["gates"]]
    f.require(len(gate_keys) == 4 and set(gate_keys) == {(k,p) for k in (64,4096) for p in ("mapped","shared")}, "incomplete gate matrix")
    for row in report["gates"]:
        stdout, stderr = checked_logs(source, row)
        metadata_check(row, stdout)
        gate_evidence(stdout, stderr, row["policy"], row["kib"], report["identity"])
        f.require(row["trace"] == [line for line in stderr.splitlines() if line.startswith(("ALLOCATE ", "FREE ", "MEMORY_SUMMARY "))], "gate trace mismatch")
    for name, digest in report["sources"].items():
        f.require(f.digest(f.ROOT / name) == digest, f"source changed: {name}")
    destination.mkdir(exist_ok=False)
    if samples:
        with (destination / "samples.csv").open("w", newline="") as out:
            writer = csv.DictWriter(out, fieldnames=list(samples[0]), lineterminator="\n")
            writer.writeheader(); writer.writerows(samples)
        report["raw_samples"] = "samples.csv"; report["raw_samples_sha256"] = f.digest(destination / "samples.csv")
    report["input_report_sha256"] = f.digest(source)
    report["export_scope"] = "fixed matrix, gates, log/source hashes, checksums, samples and allocation signatures rechecked"
    (destination / "report.json").write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    print(f"Host-access export PASS: {destination}, {len(samples)} samples")


def check_signatures(signatures, types, result, signature):
    key = (result["slots"], result["kib"], result["policy"] == "shared")
    f.require(signature == signatures.setdefault(key, signature), "copy/mapped allocation mismatch")
    # Shared allocation changes count/rounding, but must not change memory type.
    key = (result["slots"], result["kib"])
    selection = {t for _, t in signature}
    f.require(len(selection) == 1 and selection == types.setdefault(key, selection), "shared/separate memory type mismatch")


def checked_logs(source, row):
    folder = source.parent / row["label"]
    for name in ("stdout", "stderr"):
        f.require(f.digest(folder / f"{name}.txt") == row[f"{name}_sha256"], "log hash mismatch")
    stdout = (folder / "stdout.txt").read_text(); stderr = (folder / "stderr.txt").read_text()
    f.require("Validation Error:" not in stdout + stderr, "retained validation error")
    return stdout, stderr


def metadata_check(row, stdout):
    f.require(row["native_buffers"] == f.bench.records(stdout, "NATIVE_BUFFER ")
              and row["host_memory"] == f.bench.records(stdout, "HOST_MEMORY "), "native metadata mismatch")


def gate_evidence(stdout, stderr, policy, kib, identity):
    f.require("HOST_GATE disjoint CPU read/write/flush while other range pending PASS" in stdout
              and "HOST_ACCESS full outputs/guards/checksums PASS; all slots drained" in stdout, "missing gate proof")
    f.require(f.bench.records(stdout, "DEVICE ") == [identity], "gate identity mismatch")
    memory = f.bench.parse_memory(stderr); count = 1 if policy == "shared" else 2
    f.require(memory["allocations"] == memory["frees"] == memory["peak_count"] == count, "gate allocation leak")
    buffers = f.bench.records(stdout, "NATIVE_BUFFER "); properties = f.bench.records(stdout, "HOST_MEMORY ")
    f.require(len(buffers) == count and len(properties) == 1 and properties[0]["atom"] > 0, "gate buffer metadata missing")
    span = kib * 1024 + 128; atom = properties[0]["atom"]
    requested = 2 * ((span + atom - 1) // atom * atom) if policy == "shared" else span
    f.require(all(b["host"] is True and b["requested"] == requested for b in buffers), "gate size/policy mismatch")
    signature = [(a["bytes"], a["type"]) for a in f.bench.records(stderr, "ALLOCATE ")]
    f.require(signature == [(b["allocated"], b["type"]) for b in buffers], "gate backing mismatch")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--software", action="store_true", help="64-frame correctness only, no speed claim")
    parser.add_argument("--export", nargs=2, type=Path, metavar=("REPORT", "DESTINATION"))
    args = parser.parse_args()
    if args.export: export(*args.export); return
    env = os.environ.copy(); env["LD_LIBRARY_PATH"] = str(f.ROOT / "target/release") + ":" + env.get("LD_LIBRARY_PATH", "")
    build(env)
    if args.check: print("Host-access build/host tests PASS; no GPU"); return
    f.require("VK_LAYER_KHRONOS_validation" in env.get("VK_INSTANCE_LAYERS", "")
              and env.get("VK_LAYER_VALIDATE_SYNC") == "1" and not env.get("VK_LOADER_LAYERS_DISABLE"), "enable sync validation")
    f.require((env.get("VK_DRIVER_FILES") or env.get("VK_ICD_FILENAMES")) and not env.get("OGPU_TRACE_LOADER"), "select one ICD without tracing")
    dest = Path(tempfile.mkdtemp(prefix="host-", dir=f.BUILD)); print(f"Host-access artifacts: {dest}", flush=True)
    sources = [f.HERE / name for name in ("host_access.c", "host_access.py", "test_host_access.c", "test_host_access.py", "small.c", "run.py")]
    sources += [f.ROOT / name for name in ("examples/learned_image/native.c", "examples/learned_image/native_workload.h", "examples/learned_image/extent.h", "examples/learned_image/trace_memory.c", "examples/learned_image/allocation_tracker.h", "examples/learned_image/benchmark.py", "examples/learned_image/compare_native.py", "examples/learned_image/run.py", "examples/compiler/transform.generated.h", "include/ogpu.h", "vendor/Vulkan-Headers/include/vulkan/vulkan_core.h")]
    sources += sorted((f.ROOT / "crates/ogpu/src").glob("*.rs"))
    sources += sorted((f.ROOT / "examples/learned_image/generated").glob("*.h"))
    report = dict(schema=1, scope="P3 copied/mapped application producer/consumer and separate/shared native ranges",
                  revision=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=f.ROOT, text=True).strip(),
                  dirty=bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=f.ROOT, text=True)),
                  software=args.software, sources={str(p.relative_to(f.ROOT)):f.digest(p) for p in sources},
                  artifacts={n:f.digest(f.BUILD/n) for n in ("host-native", "host-ogpu", "trace-memory.so")},
                  library_sha256=f.digest(f.ROOT/"target/release/libogpu.so"),
                  environment={k:env.get(k) for k in ("VK_DRIVER_FILES", "VK_ICD_FILENAMES", "OGPU_VULKAN_LIBRARY", "VK_LAYER_PATH")},
                  gates=[], validation=[], timing=[], memory=[], complete=False)
    def save(): (dest / "report.json").write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    save(); signatures = {}; types = {}
    traced = env.copy(); traced["OGPU_TRACE_LOADER"] = env.get("OGPU_VULKAN_LIBRARY", "libvulkan.so.1")
    traced["OGPU_VULKAN_LIBRARY"] = str(f.BUILD / "trace-memory.so")
    count = 64 if args.software else 1000
    def invoke(slots, kib, policy, mode, environment, label):
        out = dest / label; out.mkdir()
        binary = f.BUILD / ("host-ogpu" if policy == "ogpu" else "host-native")
        completed = subprocess.run([str(binary), policy, str(slots), str(kib), str(count), mode],
                                   env=environment, capture_output=True, text=True, timeout=120)
        (out/"stdout.txt").write_text(completed.stdout); (out/"stderr.txt").write_text(completed.stderr)
        f.require(completed.returncode == 0 and "Validation Error:" not in completed.stdout + completed.stderr, f"host control failed: {out}")
        row = dict(label=label, stdout_sha256=f.digest(out/"stdout.txt"), stderr_sha256=f.digest(out/"stderr.txt"))
        devices = f.bench.records(completed.stdout, "DEVICE "); f.require(len(devices) == 1, "missing device")
        f.require(devices[0] == report.setdefault("identity", devices[0]), "device mismatch")
        row["native_buffers"] = f.bench.records(completed.stdout, "NATIVE_BUFFER ")
        row["host_memory"] = f.bench.records(completed.stdout, "HOST_MEMORY ")
        if mode == "gate":
            gate_evidence(completed.stdout, completed.stderr, policy, kib, report["identity"])
            row.update(kib=kib, policy=policy, trace=[line for line in completed.stderr.splitlines() if line.startswith(("ALLOCATE ", "FREE ", "MEMORY_SUMMARY "))])
        else:
            result, _, frames = parse(completed.stdout, slots, kib, policy, count, mode == "validate")
            row.update(result=result, frames=frames)
            if environment.get("OGPU_TRACE_LOADER"):
                signature = memory_check(row, completed.stderr); check_signatures(signatures, types, result, signature)
        return row
    for kib in (64,4096):
        for policy in ("mapped", "shared"):
            report["gates"].append(invoke(2,kib,policy,"gate",traced,f"gate-{kib}-{policy}")); save()
            print(f"Gated {kib} KiB {policy}: range independence PASS", flush=True)
    for slots, kib, policy in matrix():
        report["validation"].append(invoke(slots,kib,policy,"validate",traced,f"validation-{slots}-{kib}-{policy}")); save()
        print(f"Validated {slots}/{kib}/{policy}", flush=True)
    if not args.software:
        timing = f.compare_native.timing_environment(env)
        for round_index in range(3):
            for slots in (1,2):
                for ki, kib in enumerate((64,4096)):
                    policies = tuple(p for p in POLICIES if slots == 2 or p != "shared")
                    for policy in f.order(round_index + slots - 1 + ki, policies):
                        row = invoke(slots,kib,policy,"measure",timing,f"timing-{round_index}-{slots}-{kib}-{policy}")
                        row["round"] = round_index; row["statistics"] = summarize(row["frames"])
                        report["timing"].append(row); save()
                        print(f"Timed r{round_index} {slots}/{kib}/{policy}: {row['result']['wall_ms']:.3f} ms", flush=True)
        for slots,kib,policy in matrix():
            row = invoke(slots,kib,policy,"measure",traced,f"memory-{slots}-{kib}-{policy}"); row.pop("frames")
            report["memory"].append(row); save()
    report["complete"] = True; save(); print(f"Host-access PASS: {dest}")


if __name__ == "__main__": main()
