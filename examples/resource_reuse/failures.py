#!/usr/bin/env python3
"""Real-workload failure/drain integration; safe synthetic faults, no timing acceptance."""
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
spec = importlib.util.spec_from_file_location("reuse_stream", HERE/"run.py")
reuse = importlib.util.module_from_spec(spec)
spec.loader.exec_module(reuse)
f, ROOT, BUILD, require = reuse.f, reuse.ROOT, reuse.BUILD, reuse.require
MODES = ("reject", "timeout", "poll-error", "wait-error", "synthetic-loss", "unknown-submit")
FAULT_OPERATIONS = ("submit-rejected-before-driver", "poll-no-observation", "poll-no-observation",
                    "wait-error-before-drain", "synthetic-loss-after-real-drain", "submit-accepted-then-error")


def parse(stdout, stderr, mode, allocation, policy, slots):
    require(mode in range(1,7), "unknown injection mode")
    require("Validation Error:" not in stdout+stderr, "Vulkan validation error")
    require("Reuse failure/drain integration PASS; synthetic faults, not hardware-loss evidence" in stdout,
            "missing completion marker")
    context = f.bench.records(stdout, "REUSE_CONTEXT ")
    require(len(context) == 1, "missing/duplicate context")
    context = context[0]
    expected = dict(abi=18, backend="vulkan", workload="learned-image", mode=mode,
                    allocation=allocation, policy=policy, slots=slots)
    require(all(context.get(k) == v for k,v in expected.items()), "wrong failure configuration")
    require(all(context.get(k) == 1 for k in ("graphics", "compute", "buffer_address", "timeline",
                "synchronization2", "descriptor_heap", "address_commands", "untyped_pointers")), "missing enabled baseline")
    device = f.bench.records(stdout, "DEVICE ")
    require(len(device) == 1, "missing/duplicate device")
    events = f.bench.records(stdout, "REUSE_EVENT ")
    require(events and all(0 <= e["slot"] < slots and e["frame"] >= 12 and e["generation"] > 0
                          and e["retains_list"] == (policy == "compiled") for e in events), "invalid ownership labels")
    final = [e for e in events if e["operation"] == "before-teardown"]
    recovered = mode <= 3
    require([e["slot"] for e in final] == list(range(slots)), "missing/duplicate teardown slot")
    require(all(not e["pending_receipt"] and e["state"] == ("idle" if recovered else "failed-drained")
                for e in final), "unsafe teardown state")
    gate = f.bench.records(stdout, "REUSE_FAILURE_GATE ")
    require(len(gate) == 1 and gate[0].get("recovered") == recovered, "wrong recovery result")
    if recovered:
        require(gate[0].get("verified_recovery_frames") == 64 and
                "Reuse HOST padding and all slot generations retired PASS" in stdout and
                "Streaming final input/weight integrity and all intermediate guards PASS" in stdout,
                "missing recovery correctness")
    else:
        require(gate[0].get("outputs_after_error_accepted") is False, "terminal output falsely accepted")
        quarantines = [e for e in events if e["operation"] == "quarantine"]
        drains = [e for e in events if e["operation"] == "drained-no-recycle"]
        require([e["slot"] for e in quarantines] == list(range(slots)) and
                all(e["state"] == "failed-pending" for e in quarantines), "missing quarantine")
        require([e["slot"] for e in drains] == list(range(slots)) and
                all(e["state"] == "failed-drained" for e in drains), "missing drain")
    operation = "submit" if mode in (1,6) else "poll" if mode in (2,3) else "wait"
    observed = [e for e in events if e["slot"] == 0 and e["operation"] == operation]
    require(len(observed) == (2 if mode == 1 else 1), "missing/extra observation")
    error = observed[0]
    require(error["result"] == (0 if mode == 2 else -5) and
            error["native_result"] == {1:-1, 2:0, 3:-1, 4:-1, 5:-4, 6:-13}[mode], "wrong reported error")
    require(error["state"] == ("recorded" if mode in (1,6) else "pending") and
            error["pending_receipt"] == (mode not in (1,6)), "error lost ownership state")
    if mode == 1:
        require(observed[1]["result"] == 0 and observed[1]["generation"] == error["generation"]+1,
                "retry did not acquire a new generation")
        require(any(e["operation"] == "known-unsubmitted-abort" and e["state"] == "idle" for e in events),
                "missing known-unsubmitted transition")
    fault = f.bench.records(stderr, "FAULT_SUMMARY ")
    require(len(fault) == 1 and fault[0]["injections"] == 1 and fault[0]["armed"] == 0 and
            fault[0]["accepted"] == fault[0]["drained"] > 0 and
            bool(fault[0]["rejected_value"]) == (mode == 1), "injection/drain accounting failed")
    native = f.bench.records(stderr, "FAULT ")
    hits = [i for i,e in enumerate(native) if e["operation"] == FAULT_OPERATIONS[mode-1] and e["mode"] == mode]
    require(len(hits) == 1, "wrong injection point/count")
    hit = hits[0]
    if mode == 5:
        require(hit > 0 and native[hit-1]["operation"] == "queue-drain" and native[hit-1]["result"] == 0
                and native[hit-1]["drained"] == native[hit-1]["accepted"], "loss preceded physical drain")
    if mode in (4,6):
        required = "real-wait" if mode == 4 else "queue-drain"
        require(any(e["operation"] == required and e["result"] == 0 and e["drained"] >= native[hit]["accepted"]
                    for e in native[hit+1:]), "error did not drain accepted work")
    memory = f.bench.parse_memory(stderr)
    expected_count = 4+slots if allocation == "arena" else 1+8*slots
    require(memory["allocations"] == memory["frees"] == memory["peak_count"] == expected_count,
            "growth, unexpected allocation or leak")
    ranges = reuse.parse_ranges(stdout, allocation, slots)
    return dict(context=context, device=device[0], events=events, gate=gate[0], fault=fault[0], memory=memory,
                ranges=ranges, trace=[line for line in stderr.splitlines()
                    if line.startswith(("FAULT ", "ALLOCATE ", "FREE ", "MEMORY_SUMMARY ", "FAULT_SUMMARY "))])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="build/CPU gates only")
    args = parser.parse_args()
    env = os.environ.copy()
    env["LD_LIBRARY_PATH"] = str(ROOT/"target/release")+":"+env.get("LD_LIBRARY_PATH", "")
    subprocess.run([sys.executable, str(HERE/"run.py"), "--check"], cwd=ROOT, env=env, check=True)
    cc = shlex.split(env.get("CC", "cc"))
    common = [*cc, "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-Iinclude",
              "-Ivendor/Vulkan-Headers/include", "-Iexamples/learned_image/generated"]
    for name, source, flags in (("failures", "failures.c", ["-Ltarget/release", "-logpu", "-ldl"]),
                               ("failure-shim.so", "failure_shim.c", ["-fPIC", "-shared", "-Wl,-Bsymbolic", "-ldl"]),
                               ("test-failure-shim", "test_failure_shim.c", ["-ldl"])):
        subprocess.run([*common, str(HERE/source), *flags, "-o", str(BUILD/name)], cwd=ROOT, env=env, check=True)
    subprocess.run([str(BUILD/"test-failure-shim")], env=env, check=True)
    subprocess.run([sys.executable, str(HERE/"test_failures.py")], env=env, check=True)
    if args.check:
        print("Reuse failure build/CPU gates PASS; no GPU"); return
    require(env.get("VK_DRIVER_FILES") and "VK_LAYER_KHRONOS_validation" in env.get("VK_INSTANCE_LAYERS", "")
            and env.get("VK_LAYER_VALIDATE_SYNC") == "1" and not env.get("VK_LOADER_LAYERS_DISABLE"), "select driver/validation")
    require(not env.get("OGPU_TRACE_LOADER"), "no preinstalled tracing")
    env["OGPU_TRACE_LOADER"] = env.get("OGPU_VULKAN_LIBRARY", "libvulkan.so.1")
    env["OGPU_VULKAN_LIBRARY"] = str(BUILD/"failure-shim.so")
    dest = Path(tempfile.mkdtemp(prefix="failures-", dir=BUILD))
    print(f"Reuse failure evidence: {dest}", flush=True)
    extent = (65,47,131,95)
    paths, hashes = reuse.stream.fixtures(extent)
    env["OGPU_STREAM_SERIAL_A"], env["OGPU_STREAM_SERIAL_B"] = str(dest/"a.rgba"), str(dest/"b.rgba")
    env["OGPU_STREAM_ALLOCATION"], env["OGPU_STREAM_SERIAL_MODE"] = "dedicated", "write"
    sources = list(HERE.glob("*.c"))+list(HERE.glob("*.h"))+list(HERE.glob("*.py"))
    sources += [ROOT/"examples/performance_frontier"/n for n in ("stream.c","small.c","stream.py","run.py")]
    sources += [ROOT/"examples/learned_image"/n for n in ("trace_memory.c","allocation_tracker.h","benchmark.py","extent.h")]
    sources += [ROOT/"include/ogpu.h", *list((ROOT/"examples/learned_image/generated").glob("*.h"))]
    report = dict(schema=1, scope="synthetic failure/drain consumer integration; not actual hardware loss or timing",
                  revision=subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip(),
                  dirty=bool(subprocess.check_output(["git","status","--porcelain"],cwd=ROOT,text=True)),
                  complete=False, sources={str(p.relative_to(ROOT)):f.digest(p) for p in sources},
                  artifacts={n:f.digest(BUILD/n) for n in ("failures","failure-shim.so","stream")},
                  runtime_sha256=f.digest(ROOT/"target/release/libogpu.so"), fixtures=hashes,
                  resource_labels=["input","weights","hidden","denoised","processed","upload","readback"],
                  executable_labels=["hidden","denoise","process","fullscreen","display"],
                  environment={k:env.get(k) for k in ("VK_DRIVER_FILES","VK_INSTANCE_LAYERS","VK_LAYER_VALIDATE_SYNC","OGPU_TRACE_LOADER")}, runs=[])
    def save(): (dest/"report.json").write_text(json.dumps(report, indent=2, allow_nan=False)+"\n")
    def run(name, binary, policy, slots, mode=None):
        command = [str(BUILD/binary),policy,str(slots),*map(str,extent),"12","validate",*map(str,paths)]
        if mode is not None: command.append(str(mode))
        try:
            result = subprocess.run(command, env=env, capture_output=True, text=True, timeout=120)
        except subprocess.TimeoutExpired as error:
            def output(value): return value.decode(errors="replace") if isinstance(value,bytes) else value or ""
            (dest/(name+".stdout")).write_text(output(error.stdout))
            (dest/(name+".stderr")).write_text(output(error.stderr))
            report["failure"] = dict(label=name,command=command,timeout_seconds=120)
            save()
            raise RuntimeError(f"failure integration timed out: {name}; inspect {dest}") from error
        (dest/(name+".stdout")).write_text(result.stdout); (dest/(name+".stderr")).write_text(result.stderr)
        if result.returncode:
            report["failure"] = dict(label=name,command=command,returncode=result.returncode)
            save()
        require(result.returncode == 0, f"failure integration failed: {name}; inspect {dest}")
        return result
    save()
    serial = run("serial", "stream", "ogpu", 1)
    report["serial"] = reuse.parse(serial.stdout,serial.stderr,"dedicated","ogpu",1,extent,12)
    report["serial_sha256"] = [f.digest(dest/n) for n in ("a.rgba","b.rgba")]
    report["serial_logs_sha256"] = {n:f.digest(dest/("serial."+n)) for n in ("stdout","stderr")}
    env["OGPU_STREAM_SERIAL_MODE"] = "check"
    for slots in (2,3):
        for allocation in ("dedicated","arena"):
            env["OGPU_STREAM_ALLOCATION"] = allocation
            for policy in ("ogpu","compiled"):
                for mode,label in enumerate(MODES,1):
                    name = f"{allocation}-{policy}-{slots}-{label}"
                    result = run(name,"failures",policy,slots,mode)
                    try:
                        row = parse(result.stdout,result.stderr,mode,allocation,policy,slots)
                    except (RuntimeError,ValueError,KeyError) as error:
                        report["failure"] = dict(label=name,validation_error=str(error))
                        save()
                        raise
                    require(row["device"] == report["serial"]["device"], "device changed")
                    row.update(label=name, stdout_sha256=f.digest(dest/(name+".stdout")), stderr_sha256=f.digest(dest/(name+".stderr")))
                    report["runs"].append(row); save()
                    print(f"Validated {name}", flush=True)
    report["complete"] = True; save()
    print(f"Reuse failure integration PASS: {dest}")


if __name__ == "__main__": main()
