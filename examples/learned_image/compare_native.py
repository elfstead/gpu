#!/usr/bin/env python3
"""Fresh, matched native Vulkan/OGPU serialized-latency comparison."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
sys.dont_write_bytecode = True
import benchmark as bench
import run
import run_native

ENGINES = ("native", "ogpu")


def timing_environment(validation):
    run.require(not validation.get("OGPU_TRACE_LOADER"), "tracing must not enter timing")
    environment = validation.copy()
    for key in ("VK_INSTANCE_LAYERS", "VK_LAYER_VALIDATE_SYNC", "VK_LOADER_LAYERS_ENABLE"):
        environment.pop(key, None)
    environment["VK_LOADER_LAYERS_DISABLE"] = "*"
    return environment


def checked_samples(stdout, extent, mode, identity):
    metadata, samples = bench.parse_samples(stdout, mode)
    bench.check_metadata(metadata, extent, mode, False)
    run.require(run_native.identity_evidence(stdout) == identity, "timed device/clock differs from validated control")
    run.require(all((s["device_ms"] is not None) == identity["clock"]["supported"] for s in samples),
                "sample clock support disagrees with device")
    return metadata, samples


def control_order(round_index, extent_index, mode_index):
    return ENGINES if (round_index + extent_index + mode_index) % 2 == 0 else tuple(reversed(ENGINES))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="build and host tests only")
    args = parser.parse_args()
    os.chdir(run.ROOT)
    run.command(sys.executable, str(run.HERE / "run_native.py"), "--workload" if args.check else "--scale", "--check")
    run.command(sys.executable, str(run.HERE / "test_benchmark.py"))
    run.command(sys.executable, str(run.HERE / "test_compare_native.py"))
    if args.check:
        print("Native/OGPU comparison build and host tests PASS; no GPU")
        return
    validation = os.environ.copy()
    run.require("VK_LAYER_KHRONOS_validation" in validation.get("VK_INSTANCE_LAYERS", "")
                and validation.get("VK_LAYER_VALIDATE_SYNC") == "1"
                and not validation.get("VK_LOADER_LAYERS_DISABLE"), "enable synchronization validation")
    run.require(validation.get("VK_DRIVER_FILES") or validation.get("VK_ICD_FILENAMES"), "select one ICD explicitly")
    run.require(not validation.get("OGPU_TRACE_LOADER"), "start with real loader, not trace shim")
    build = run.ROOT / "target/learned-image/native-control"
    validated_path, validated = run_native.workload(build, validation, True)
    destination = Path(tempfile.mkdtemp(prefix="comparison-", dir=build))
    print(f"Matched comparison artifacts: {destination}", flush=True)
    identity = validated["runs"][0]["identity"]
    report = dict(schema=1, scope="matched native/OGPU one-frame serialized latency; not isolated API overhead or peak throughput",
                  revision=validated["revision"], dirty=validated["dirty"], model_sha256=validated["model_sha256"],
                  source_sha256=validated["source_sha256"], artifacts=validated["artifacts"],
                  environment=validated["environment"], identity=identity,
                  validation_report=str(validated_path / "report.json"),
                  validation_sha256=run.digest_file(validated_path / "report.json"),
                  runs=[], memory=[])
    for name in ("compare_native.py", "test_compare_native.py", "benchmark.py", "export_comparison.py"):
        report["source_sha256"][name] = run.digest_file(run.HERE / name)
    def save():
        (destination / "report.json").write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    save()
    timing = timing_environment(validation)
    report["timing_policy"] = dict(warmups=10, samples=30, processes_per_control=3, frames_in_flight=1,
        validation=False, allocation_tracing=False, mode_order="alternate by round",
        engine_order="alternate by round+extent_index+mode_index", query_bits=identity["clock"]["bits"])
    selected = []
    for extent in bench.EXTENTS:
        label = "x".join(str(v) for v in extent)
        reference = build.parent / "reference-scale"
        manifest = json.loads((reference / "manifest.json").read_text())
        cases = [c for c in manifest["cases"] if tuple(c["input_size"] + c["output_size"]) == extent
                 and c["seed"] in (2001, 2002)][:2]
        run.require(len(cases) == 2 and [c["seed"] for c in cases] == [2001, 2002], "missing A/B cases")
        baseline = validated_path / label / "native-original-end-to-end" / "normal-1-final.rgba"
        baseline_hash = run.digest_file(baseline)
        selected.append((extent, label, reference, cases, baseline_hash))
    def invoke(engine, mode, extent, label, reference, cases, baseline_hash, output, environment):
        executable = build / "native-original" if engine == "native" else build.parent / "app-original"
        stdout, stderr = bench.invoke(executable, "--measure", mode, extent, reference, cases, output, environment)
        metadata, samples = checked_samples(stdout, extent, mode, identity)
        final_hash = run.digest_file(output / "normal-39-final.rgba")
        run.require(final_hash == baseline_hash, "measured/traced final B differs from fully validated B")
        return dict(extent=extent, engine=engine, mode=mode, metadata=metadata, final_sha256=final_hash,
                    samples=samples, statistics=bench.summarize(samples)), stdout, stderr
    for round_index in range(3):
        for extent_index, (extent, label, reference, cases, baseline_hash) in enumerate(selected):
            for mode_index, mode in enumerate(bench.MODES if round_index % 2 == 0 else tuple(reversed(bench.MODES))):
                for engine in control_order(round_index, extent_index, mode_index):
                    output = destination / label / f"run-{round_index}-{engine}-{mode}"
                    row, _, _ = invoke(engine, mode, extent, label, reference, cases, baseline_hash, output, timing)
                    row["round"] = round_index
                    report["runs"].append(row); save()
                    print(f"Paired {label} {mode} round {round_index} {engine}: {row['statistics']['total_ms']['median']:.3f} ms", flush=True)
    traced = validation.copy()
    traced["OGPU_TRACE_LOADER"] = validation.get("OGPU_VULKAN_LIBRARY", "libvulkan.so.1")
    traced["OGPU_VULKAN_LIBRARY"] = str(build / "trace-memory.so")
    for extent, label, reference, cases, baseline_hash in selected:
        for mode in bench.MODES:
            baseline_memory = None
            for engine in ENGINES:
                output = destination / label / f"memory-{engine}-{mode}"
                row, stdout, stderr = invoke(engine, mode, extent, label, reference, cases, baseline_hash, output, traced)
                # Deliberately discard traced timings; only ordinary runs enter distributions.
                memory = run_native.allocation_evidence(stdout, stderr, row["metadata"], engine)
                if baseline_memory is None:
                    baseline_memory = memory["signature"]
                else:
                    run.require(memory["signature"] == baseline_memory, "timing-mode native/OGPU allocation mismatch")
                report["memory"].append(dict(extent=extent, engine=engine, mode=mode, requested=row["metadata"],
                    final_sha256=row["final_sha256"], allocation=memory))
                save()
                print(f"Matched memory {label} {mode} {engine}: {memory['summary']['peak_bytes']} bytes; all freed", flush=True)
    report["complete"] = True; save()
    print(f"Matched native/OGPU comparison PASS: {destination}")


if __name__ == "__main__":
    main()
