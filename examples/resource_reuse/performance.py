#!/usr/bin/env python3
"""Matched mapped-range native/public strategy comparison, not general API performance approval."""
import argparse
import csv
import importlib.util
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile
sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("reuse",HERE/"run.py")
reuse = importlib.util.module_from_spec(spec); spec.loader.exec_module(reuse)
f, ROOT, BUILD, require = reuse.f, reuse.ROOT, reuse.BUILD, reuse.require
CONTROLS = tuple((a,p) for a in ("dedicated","arena") for p in ("ogpu","reset","compiled","replay"))
EXTENTS = ((65,47,131,95),(1919,1079,2561,1441))
ROUNDS, FRAMES, VALIDATE = 4, 500, 64


def order(round_index, extent_index, slots):
    shift = (2*round_index+extent_index+slots-1)%len(CONTROLS)
    controls = CONTROLS[shift:]+CONTROLS[:shift]
    return controls if round_index%2 == 0 else tuple(reversed(controls))


def parse(stdout, stderr, allocation, policy, slots, extent, frames, validation, traced):
    result, device, samples = reuse.stream.parse(stdout,policy,slots,extent,frames,validation)
    require("Validation Error:" not in stdout+stderr, "validation error")
    require("Reuse HOST padding and all slot generations retired PASS" in stdout, "missing range retirement gate")
    ranges = reuse.parse_ranges(stdout,allocation,slots)
    backing = f.bench.records(stdout,"REUSE_MEMORY ")
    cpu = f.bench.records(stdout,"REUSE_CPU_STORAGE ")
    require(len(backing) == len(cpu) == 1, "missing/duplicate storage accounting")
    expected = 4+slots if allocation == "arena" else 1+8*slots
    require(backing[0]["allocation"] == allocation and backing[0]["buffer_allocations"] == expected-slots,
            "wrong backing strategy")
    require(cpu[0]["sample_bytes"] == max(frames,100)*7*8 and cpu[0]["stream_stack_bytes"] > 0
            and cpu[0]["fixture_bytes"] > 0 and cpu[0]["runtime_command_bytes"] is None
            and cpu[0]["driver_command_bytes"] is None, "invalid CPU storage accounting")
    row = dict(result=result,device=device,backing=backing[0],ranges=ranges,cpu_storage=cpu[0])
    if traced:
        row["memory"] = f.bench.parse_memory(stderr)
        require(row["memory"]["allocations"] == row["memory"]["frees"] == row["memory"]["peak_count"] == expected,
                "growth, leak or unexpected backing count")
        row["allocation_signature"] = [(a["bytes"],a["type"]) for a in f.bench.records(stderr,"ALLOCATE ")]
        row["trace"] = [line for line in stderr.splitlines() if line.startswith(("ALLOCATE ","FREE ","MEMORY_SUMMARY "))]
    else:
        require(not any(word in stderr for word in ("ALLOCATE ","FREE ","FAULT ","MEMORY_SUMMARY ")), "tracing entered timing")
    if not validation: row["statistics"] = reuse.stream.summarize(samples)
    return row,samples


def comparisons(rows):
    """Use processes as repetitions; do not pool frames into independent experiments."""
    groups = {}
    for row in rows:
        r = row["result"]; key = (tuple(r["extent"]),r["slots"],row["backing"]["allocation"],r["policy"])
        groups.setdefault(key,[]).append(row)
    output = []
    for key, values in groups.items():
        require(len(values) == ROUNDS and {v["round"] for v in values} == set(range(ROUNDS)), "missing/duplicate rounds")
        wall = [v["result"]["wall_ms"]/v["result"]["frames"] for v in values]
        output.append(dict(extent=list(key[0]),slots=key[1],allocation=key[2],policy=key[3],
            wall_ms_per_frame=statistics.median(wall),wall_process_range=[min(wall),max(wall)],
            setup_ms=statistics.median(v["result"]["setup_ms"] for v in values),
            host_record_submit_ms=statistics.median(v["statistics"]["record_submit_ms"]["median"] for v in values),
            latency_ms=statistics.median(v["statistics"]["latency_ms"]["median"] for v in values),
            latency_p95_ms=statistics.median(v["statistics"]["latency_ms"]["p95"] for v in values)))
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check",action="store_true")
    parser.add_argument("--software",action="store_true",help="small correctness matrix only, no timing")
    args = parser.parse_args()
    env = os.environ.copy(); env["LD_LIBRARY_PATH"] = str(ROOT/"target/release")+":"+env.get("LD_LIBRARY_PATH","")
    subprocess.run([sys.executable,str(HERE/"run.py"),"--check"],env=env,cwd=ROOT,check=True)
    subprocess.run([sys.executable,str(HERE/"test_performance.py")],env=env,cwd=ROOT,check=True)
    if args.check: print("Reuse performance build/CPU gates PASS; no GPU"); return
    require(env.get("VK_DRIVER_FILES") and "VK_LAYER_KHRONOS_validation" in env.get("VK_INSTANCE_LAYERS","")
            and env.get("VK_LAYER_VALIDATE_SYNC") == "1" and not env.get("VK_LOADER_LAYERS_DISABLE")
            and not env.get("OGPU_TRACE_LOADER"), "select one driver; validation on, no tracing shim")
    timed = f.compare_native.timing_environment(env)
    traced = env.copy(); traced["OGPU_TRACE_LOADER"] = env.get("OGPU_VULKAN_LIBRARY","libvulkan.so.1")
    traced["OGPU_VULKAN_LIBRARY"] = str(f.BUILD/"trace-memory.so")
    extents = EXTENTS[:1] if args.software else EXTENTS
    dest = Path(tempfile.mkdtemp(prefix="performance-",dir=BUILD))
    print(f"Reuse performance evidence: {dest}",flush=True)
    sources = [*HERE.glob("*.c"),*HERE.glob("*.h"),*HERE.glob("*.py")]
    sources += [ROOT/"examples/performance_frontier"/n for n in ("small.c","stream.c","stream.py","run.py")]
    sources += [ROOT/"examples/learned_image"/n for n in ("native.c","native_workload.h","extent.h","trace_memory.c",
                "allocation_tracker.h","benchmark.py","compare_native.py")]
    sources += [*list((ROOT/"examples/learned_image/generated").glob("*.h")), ROOT/"include/ogpu.h",
                ROOT/"vendor/Vulkan-Headers/include/vulkan/vulkan_core.h"]
    report = dict(schema=1,scope="matched range allocation/submission strategies; not general API/native parity",
        revision=subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip(),
        dirty=bool(subprocess.check_output(["git","status","--porcelain"],cwd=ROOT,text=True)),complete=False,software=args.software,
        sources={str(p.relative_to(ROOT)):f.digest(p) for p in sources},
        artifacts={n:f.digest(BUILD/n) for n in ("stream","stream-native-ranges")},
        runtime_sha256=f.digest(ROOT/"target/release/libogpu.so"),tracer_sha256=f.digest(f.BUILD/"trace-memory.so"),
        environments={name:{k:e.get(k) for k in ("VK_DRIVER_FILES","VK_INSTANCE_LAYERS","VK_LAYER_VALIDATE_SYNC",
            "VK_LOADER_LAYERS_DISABLE","OGPU_TRACE_LOADER","OGPU_VULKAN_LIBRARY")} for name,e in (("validation",traced),("timing",timed))},
        protocol=dict(validation_frames=VALIDATE,frames=FRAMES,warmups=100,rounds=ROUNDS,
            order="rotate by 2*round+extent_index+slots-1, reverse odd rounds", clocks="monotonic wall/CPU intervals; no device queries or clock locking"),
        fixtures={},serial=[],validation=[],timing=[],memory=[])
    def save(): (dest/"report.json").write_text(json.dumps(report,indent=2,allow_nan=False)+"\n")
    save(); identity = None; signatures = {}; layouts = {}; budgets = {}
    fixture_paths = {}
    def invoke(extent,allocation,policy,slots,count,validate,environment,label,write=False):
        nonlocal identity
        actual = environment.copy(); prefix = "x".join(map(str,extent))
        actual.update(OGPU_STREAM_ALLOCATION=allocation,OGPU_STREAM_SERIAL_MODE="write" if write else "check",
            OGPU_STREAM_SERIAL_A=str(dest/(prefix+"-a.rgba")),OGPU_STREAM_SERIAL_B=str(dest/(prefix+"-b.rgba")))
        binary = "stream-native-ranges" if policy in ("reset","replay") else "stream"
        result = subprocess.run([str(BUILD/binary),policy,str(slots),*map(str,extent),str(count),
            "validate" if validate else "measure",*map(str,fixture_paths[extent])],env=actual,capture_output=True,text=True,timeout=240)
        (dest/(label+".stdout")).write_text(result.stdout); (dest/(label+".stderr")).write_text(result.stderr)
        require(result.returncode == 0, f"control failed: {label}; inspect {dest}")
        row,samples = parse(result.stdout,result.stderr,allocation,policy,slots,extent,count,validate,bool(actual.get("OGPU_TRACE_LOADER")))
        if identity is None: identity = row["device"]; report["identity"] = identity
        require(row["device"] == identity,"device changed")
        key = (extent,allocation,slots)
        require(row["ranges"] == layouts.setdefault(key,row["ranges"]),"native/public layout mismatch")
        require(row["backing"]["buffer_requested_bytes"] == budgets.setdefault((extent,slots),row["backing"]["buffer_requested_bytes"]),
                "dedicated/arena byte budgets differ")
        if "allocation_signature" in row:
            require(row["allocation_signature"] == signatures.setdefault(key,row["allocation_signature"]),"native/public size/type mismatch")
        row.update(label=label,stdout_sha256=f.digest(dest/(label+".stdout")),stderr_sha256=f.digest(dest/(label+".stderr")))
        if samples:
            path = dest/(label+".csv")
            with path.open("w",newline="") as output:
                writer = csv.DictWriter(output,fieldnames=list(samples[0]),lineterminator="\n")
                writer.writeheader(); writer.writerows(samples)
            row.update(samples_file=path.name,samples_sha256=f.digest(path))
        return row
    for extent in extents:
        paths,hashes = reuse.stream.fixtures(extent); fixture_paths[extent] = paths
        prefix = "x".join(map(str,extent)); report["fixtures"][prefix] = hashes
        serial = invoke(extent,"dedicated","ogpu",1,12,True,traced,prefix+"-serial",True)
        serial["serial_sha256"] = [f.digest(dest/(prefix+"-"+n+".rgba")) for n in ("a","b")]
        report["serial"].append(serial); save()
        for slots in (1,2,3):
            for allocation,policy in CONTROLS:
                label = f"validation-{prefix}-{slots}-{allocation}-{policy}"
                report["validation"].append(invoke(extent,allocation,policy,slots,VALIDATE,True,traced,label)); save()
                print(f"Validated {label}",flush=True)
    if not args.software:
        for round_index in range(ROUNDS):
            for extent_index,extent in enumerate(extents):
                prefix = "x".join(map(str,extent))
                for slots in (1,2,3):
                    for allocation,policy in order(round_index,extent_index,slots):
                        label = f"timing-{round_index}-{prefix}-{slots}-{allocation}-{policy}"
                        row = invoke(extent,allocation,policy,slots,FRAMES,False,timed,label)
                        row["round"] = round_index; report["timing"].append(row); save()
                        print(f"Timed {label}: {row['result']['wall_ms']/FRAMES:.6f} ms/frame",flush=True)
        # Separate traced runs use the same warmup/measurement lifecycle, not timing evidence.
        for extent in extents:
            prefix = "x".join(map(str,extent))
            for slots in (1,2,3):
                for allocation,policy in CONTROLS:
                    label = f"memory-{prefix}-{slots}-{allocation}-{policy}"
                    row = invoke(extent,allocation,policy,slots,64,False,traced,label)
                    report["memory"].append(row); save()
        report["comparisons"] = comparisons(report["timing"])
    report["complete"] = True; save()
    print(f"Reuse performance suite PASS: {dest}")


if __name__ == "__main__": main()
