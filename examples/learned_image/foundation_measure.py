#!/usr/bin/env python3
"""Check explicit replay timing; no matched-native performance acceptance claim."""
import argparse
import json
import math
import os
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile
sys.dont_write_bytecode = True
import foundation

run, bench = foundation.acceptance, foundation.benchmark
PHASES = ("upload_ms", "submit_ms", "wait_ms", "query_ms", "read_ms")
FIELDS = (*PHASES, "total_ms", "device_ms")


def metadata(text, mode, extent, validation):
    rows = bench.records(text,"REPLAY ")
    run.require(len(rows) == 1,"missing/duplicate replay metadata")
    m = rows[0]
    w,h,ow,oh = extent
    input_bytes,final = w*h*4+128,ow*oh*4
    device = w*h*40+ow*oh*16+356+5*128
    readback = final+128+device if validation else (0 if mode == "resident" else final+128)
    timed = m["timestamp_bits"] != 0
    expected = dict(schema=1,mode=mode,validation=validation,warmups=0 if validation else 10,frames=3 if validation else 30,
                    work_lists=2 if mode == "resident" else 1,upload_bytes=0 if mode == "resident" else input_bytes,
                    readback_bytes=0 if mode == "resident" else final,query_bytes=32 if timed else 0,
                    device_linear_bytes=device+(input_bytes if mode == "resident" else 0),
                    host_linear_bytes=max(input_bytes,484)+readback+16+(32 if timed else 0),
                    cpu_payload_bytes=484+2*input_bytes,image_logical_bytes=final)
    run.require(all(type(m.get(k)) is type(v) and m[k] == v for k,v in expected.items()),"replay policy/accounting mismatch")
    run.require(all(bench.finite(m[f]) for f in ("setup_ms","record_ms","timestamp_period_ns")),"invalid setup/clock")
    bits,period = m["timestamp_bits"],m["timestamp_period_ns"]
    run.require(type(bits) is int and 0 <= bits <= 64,"invalid timestamp width")
    wrap = period*(2.**bits)/1e6
    run.require((not timed and period == 0) or (timed and period > 0 and math.isfinite(wrap) and wrap > 10000),"invalid timestamp period")
    return m


def parse(text, mode, extent):
    m = metadata(text,mode,extent,False)
    samples = bench.records(text,"SAMPLE ")
    run.require(len(samples) == 30,"incomplete replay samples")
    for frame,s in enumerate(samples,10):
        run.require(type(s["frame"]) is int and s["frame"] == frame and type(s["input"]) is int and s["input"] == frame%2,"sample order/input mismatch")
        run.require(all(bench.finite(s[f]) for f in (*PHASES,"total_ms")),"invalid host time")
        run.require(s["total_ms"] > 0 and abs(sum(s[f] for f in PHASES)-s["total_ms"]) <= 2e-5,"incomplete host intervals")
        run.require((not m["timestamp_bits"] and s["device_ms"] is None) or
                    (m["timestamp_bits"] and bench.finite(s["device_ms"]) and s["device_ms"] > 0),"invalid device time")
        if m["timestamp_bits"]:
            wrap = m["timestamp_period_ns"]*(2.**m["timestamp_bits"])/1e6
            run.require(s["device_ms"] < wrap and s["total_ms"] < wrap,"ambiguous timestamp wrap")
    return m,samples


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check",action="store_true",help="host tests and build only")
    args = parser.parse_args()
    os.chdir(run.ROOT)
    run.command(sys.executable,"-B",str(run.HERE / "foundation.py"),"--check")
    if args.check: return
    environment = os.environ.copy()
    run.require("VK_LAYER_KHRONOS_validation" in environment.get("VK_INSTANCE_LAYERS","")
                and environment.get("VK_LAYER_VALIDATE_SYNC") == "1" and not environment.get("VK_LOADER_LAYERS_DISABLE"),
                "enable Vulkan synchronization validation for acceptance")
    run.require(not environment.get("OGPU_TRACE_LOADER"),"use the real Vulkan loader")
    build = run.ROOT / "target/learned-image"
    executables = {v:build / ("app-"+v) for v in ("foundation-original","foundation-mutated","installed-control")}
    extent,reference,cases = foundation.reuse_fixture(build)
    destination = Path(tempfile.mkdtemp(prefix="foundation-timing-",dir=build))
    print(f"Foundation timing artifacts: {destination}",flush=True)
    report = dict(schema=1,complete=False,scope="Serialized replay instrumentation only; no native comparison or throughput gate",
                  revision=subprocess.check_output(["git","rev-parse","HEAD"],text=True).strip(),
                  dirty=bool(subprocess.check_output(["git","status","--porcelain"],text=True)),
                  extent=extent,validation=[],runs=[],
                  environment={k:environment.get(k) for k in ("VK_DRIVER_FILES","OGPU_VULKAN_LIBRARY","VK_LAYER_PATH")})
    sources = [run.HERE / name for name in ("foundation.c","foundation.py","foundation_timing.h","foundation_measure.py","extent.h","model.json","run.py","benchmark.py")]
    sources += sorted((run.HERE / "generated-foundation").glob("*.h"))
    sources += [run.ROOT / "include/ogpu_next.h",run.ROOT / "target/release/libogpu.so",*executables.values()]
    report["sha256"] = {str(p.relative_to(run.ROOT)):run.digest_file(p) for p in sources}
    def save():
        (destination / "report.json").write_text(json.dumps(report,indent=2,allow_nan=False)+"\n")
    save()
    report["validation"] = foundation.validate_reuse(build,executables)
    for v in report["validation"]:
        if v["variant"].startswith("foundation-"):
            metadata((Path(v["output"]) / "stdout.txt").read_text(),v["mode"],extent,True)
    save()
    # Timings are deliberately separate from instrumented validation/snapshots.
    environment.pop("VK_INSTANCE_LAYERS",None)
    environment.pop("VK_LAYER_VALIDATE_SYNC",None)
    environment["VK_LOADER_LAYERS_DISABLE"] = "~all~"
    for variant in ("foundation-original","foundation-mutated"):
        for mode in bench.MODES:
            output = destination / (variant+"-"+mode)
            stdout,_ = bench.invoke(executables[variant],"--measure",mode,extent,reference,cases,output,environment)
            m,samples = parse(stdout,mode,extent)
            run.require(not list(output.glob("*.rgba")) and not list(output.glob("*.f32")),"timing unexpectedly wrote diagnostic snapshots")
            summary = {field:statistics.median([s[field] for s in samples]) if samples[0][field] is not None else None for field in FIELDS}
            report["runs"].append(dict(variant=variant,mode=mode,metadata=m,samples=samples,median_ms=summary,output=str(output)))
            save()
    report["complete"] = True
    save()
    print("Explicit timestamp resolve, serial A/B replay and host accounting PASS; no performance comparison claimed")


if __name__ == "__main__": main()
