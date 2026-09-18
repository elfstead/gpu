#!/usr/bin/env python3
"""Validate and measure the learned-image OGPU baseline; no native comparison yet."""
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
import run

HERE, ROOT = run.HERE, run.ROOT
EXTENTS = (run.SCALE_EXTENTS[0], run.SCALE_EXTENTS[1], run.SCALE_EXTENTS[3], run.SCALE_EXTENTS[4])
MODES = ("resident", "end-to-end")
FIELDS = ("upload_ms", "record_ms", "submit_ms", "wait_ms", "query_ms", "retire_ms", "read_ms", "total_ms", "device_ms")


def records(text, prefix):
    return [json.loads(line[len(prefix):]) for line in text.splitlines() if line.startswith(prefix)]


def finite(value):
    return type(value) in (int, float) and math.isfinite(value) and value >= 0


def parse_samples(text, mode):
    metadata = records(text, "MEASUREMENT ")
    samples = records(text, "SAMPLE ")
    run.require(len(metadata) == 1, "missing/duplicate measurement metadata")
    m = metadata[0]
    run.require(m["mode"] == mode and m["validation"] is False and m["warmups"] == 10 and m["frames"] == 30,
                "measurement mode/count mismatch")
    run.require(finite(m["setup_ms"]) and len(samples) == 30, "incomplete measurement")
    for frame, sample in enumerate(samples, 10):
        run.require(sample["frame"] == frame and sample["input"] == frame % 2, "sample order/input mismatch")
        for field in FIELDS:
            run.require((field == "device_ms" and sample[field] is None) or finite(sample[field]),
                        f"invalid {field}")
        run.require(sample["total_ms"] > 0, "zero frame duration")
        run.require(abs(sum(sample[f] for f in FIELDS[:7]) - sample["total_ms"]) <= 2e-5,
                    "component clocks do not cover frame")
    run.require(all(s["device_ms"] is None for s in samples)
                or all(s["device_ms"] is not None and s["device_ms"] > 0 for s in samples),
                "inconsistent/zero device timing")
    return m, samples


def summarize(samples):
    result = {}
    for field in FIELDS:
        values = [s[field] for s in samples if s[field] is not None]
        result[field] = (dict(mean=statistics.fmean(values), median=statistics.median(values),
                              p95=sorted(values)[math.ceil(.95 * len(values)) - 1],
                              minimum=min(values), maximum=max(values)) if values else None)
    return result


def parse_memory(text):
    live = {}
    allocations = frees = peak_bytes = peak_count = live_bytes = 0
    for line in text.splitlines():
        if line.startswith("ALLOCATE "):
            a = json.loads(line[len("ALLOCATE "):])
            run.require(type(a["id"]) is int and a["id"] > 0 and a["id"] not in live,
                        "duplicate/invalid native allocation")
            run.require(type(a["bytes"]) is int and a["bytes"] > 0, "invalid allocation size")
            run.require(type(a["type"]) is int and a["type"] >= 0, "invalid memory type")
            live[a["id"]] = a["bytes"]
            live_bytes += a["bytes"]
            allocations += 1
            peak_bytes, peak_count = max(peak_bytes, live_bytes), max(peak_count, len(live))
        elif line.startswith("FREE "):
            a = json.loads(line[len("FREE "):])
            run.require(a["id"] in live, "unknown/double native free")
            live_bytes -= live.pop(a["id"])
            frees += 1
    summary = records(text, "MEMORY_SUMMARY ")
    expected = dict(allocations=allocations, frees=frees, peak_bytes=peak_bytes,
                    peak_count=peak_count, live_bytes=live_bytes, live_count=len(live))
    run.require(allocations > 0 and not live and len(summary) == 1 and summary[0] == expected,
                "incomplete/inconsistent native allocation trace")
    return expected


def check_metadata(m, extent, mode, validation):
    w, h, ow, oh = extent
    input_bytes, final = w * h * 4 + 128, ow * oh * 4
    upload = max(input_bytes, 484)
    device = w * h * 40 + ow * oh * 16 + 356 + 5 * 128
    readback = final + 128 + (device if validation else 0)
    expected = dict(mode=mode, validation=validation, warmups=0 if validation else 10,
                    frames=3 if validation else 30,
                    device_buffers=device + (input_bytes if mode == "resident" else 0),
                    host_buffers=upload + readback + 16,
                    cpu_payload=2 * upload + 484 + readback, image_logical=final,
                    upload_bytes=0 if mode == "resident" else input_bytes,
                    readback_bytes=0 if mode == "resident" else final)
    run.require(all(m[k] == v for k, v in expected.items()), "buffer/transfer accounting mismatch")


def output_files():
    return ([f"{mode}-{frame}-final.rgba" for mode in ("normal", "diagnostic") for frame in range(3)]
            + [f"diagnostic-{frame}-{stage}.f32" for frame in range(3)
               for stage in ("input", "weights", "hidden", "denoised", "processed")])


def check_validation(output, reference, cases):
    maximum = ratio = pixel = 0
    for frame, case in enumerate(cases):
        oracle = reference / case["name"]
        normal, diagnostic = (output / f"{mode}-{frame}-final.rgba" for mode in ("normal", "diagnostic"))
        run.require(run.same_file(normal, diagnostic), "diagnostic changed pixels")
        pixel = max(pixel, run.compare_files(normal, oracle / "final.rgba"))
        for stage in ("hidden", "denoised", "processed"):
            error, bound = run.compare_files(output / f"diagnostic-{frame}-{stage}.f32",
                                             oracle / f"{stage}.f64", floating=True)
            maximum, ratio = max(maximum, error), max(ratio, bound)
        for stage in ("input", "weights"):
            expected = oracle / "input.f32" if stage == "input" else reference / "weights.f32"
            run.require(run.same_file(output / f"diagnostic-{frame}-{stage}.f32", expected), f"changed {stage}")
    for name in output_files():
        if "-2-" in name:
            run.require(run.same_file(output / name, output / name.replace("-2-", "-0-")), "A reuse mismatch")
    run.require(not run.same_file(output / "normal-0-final.rgba", output / "normal-1-final.rgba"), "A/B not distinguished")
    return dict(max_error=maximum, max_ratio=ratio, pixel_delta=pixel)


def invoke(executable, flag, mode, extent, reference, cases, output, environment):
    output.mkdir(parents=True)
    args = [str(executable), flag, mode, str(reference / "weights.f32"),
            *(str(v) for v in extent), str(output), *(str(reference / c["name"]) for c in cases)]
    p = subprocess.run(args, env=environment, capture_output=True, text=True)
    (output / "stdout.txt").write_text(p.stdout)
    (output / "stderr.txt").write_text(p.stderr)
    run.require(p.returncode == 0, f"application failed ({p.returncode}): {output}")
    run.require("Validation Error:" not in p.stdout + p.stderr, f"validation error: {output}")
    return p.stdout, p.stderr


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="build and host tests only")
    parser.add_argument("--validate-only", action="store_true", help="skip timing/allocation runs")
    args = parser.parse_args()
    os.chdir(ROOT)
    run.command(sys.executable, str(HERE / "run.py"), "--check")
    run.command(sys.executable, str(HERE / "test_benchmark.py"))
    build = ROOT / "target/learned-image"
    cc = shlex.split(os.getenv("CC", "cc"))
    run.command(*cc, "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror",
                str(HERE / "test_allocation_tracker.c"), "-o", str(build / "test-allocation-tracker"))
    run.command(str(build / "test-allocation-tracker"))
    flags = shlex.split(subprocess.check_output(["pkg-config", "--cflags", "vulkan"], text=True))
    shim = build / "trace-memory.so"
    run.command(*cc, "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-fPIC", "-shared",
                "-Wl,-Bsymbolic", *flags, str(HERE / "trace_memory.c"), "-ldl", "-o", str(shim))
    if args.check:
        print("Measurement build, parser and allocation-tracker checks PASS; no GPU")
        return
    validation = os.environ.copy()
    run.require("VK_LAYER_KHRONOS_validation" in validation.get("VK_INSTANCE_LAYERS", "")
                and validation.get("VK_LAYER_VALIDATE_SYNC") == "1"
                and not validation.get("VK_LOADER_LAYERS_DISABLE"), "enable Vulkan synchronization validation first")
    run.require(not validation.get("OGPU_TRACE_LOADER"), "start with the real Vulkan loader, not a trace shim")
    run.export_scale_reference(build)
    destination = Path(tempfile.mkdtemp(prefix="measurement-", dir=build))
    print(f"Measurement artifacts: {destination}", flush=True)
    report = dict(schema=1, revision=subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
                  dirty=bool(subprocess.check_output(["git", "status", "--porcelain"], text=True)),
                  model_sha256=run.digest_file(HERE / "model.json"),
                  scope="OGPU serialized latency; native control not implemented", validation=[], runs=[], memory=[])
    source_names = ("app.c", "benchmark.py", "run.py", "extent.h", "trace_memory.c", "allocation_tracker.h")
    report["source_sha256"] = {name: run.digest_file(HERE / name) for name in source_names}
    report["artifact_sha256"] = {p.name: run.digest_file(p) for p in
                                 [*sorted((HERE / "generated").glob("*.h")), build / "app-original",
                                  build / "app-mutated", ROOT / "target/release/libogpu.so", shim]}
    def save():
        (destination / "report.json").write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    save()
    selected = []
    # One small case proves interface mutation in both modes; four large groups
    # establish CPU gates once, then exact byte equality proves the other mode.
    for extent in ((65, 47, 131, 95), *EXTENTS):
        small = extent[0] == 65
        reference = build / ("reference" if small else "reference-scale")
        manifest = json.loads((reference / "manifest.json").read_text())
        cases = [c for c in manifest["cases"] if tuple(c["input_size"] + c["output_size"]) == extent
                 and c["seed"] in (2001, 2002)][:2]
        run.require(len(cases) == 2 and cases[0]["seed"] == 2001 and cases[1]["seed"] == 2002, "missing A/B fixtures")
        run.verified(HERE / "model.json", manifest["model_sha256"])
        run.verified(reference / "weights.f32", manifest["weights_sha256"])
        for case in cases:
            for name, digest in case["sha256"].items():
                run.verified(reference / case["name"] / name, digest)
        label = "x".join(str(v) for v in extent)
        baseline = None
        for variant in (("original", "mutated") if small else ("original",)):
            for mode in ("end-to-end", "resident"):
                output = destination / label / f"validate-{variant}-{mode}"
                stdout, _ = invoke(build / f"app-{variant}", "--validate", mode, extent, reference, cases, output, validation)
                metadata = records(stdout, "MEASUREMENT ")
                run.require(len(metadata) == 1, "validation metadata missing")
                check_metadata(metadata[0], extent, mode, True)
                if baseline is None:
                    errors = check_validation(output, reference, cases)
                    baseline = output
                else:
                    for name in output_files():
                        run.require(run.same_file(output / name, baseline / name), f"mode/interface mismatch: {output / name}")
                report["validation"].append(dict(extent=extent, variant=variant, mode=mode, errors=errors))
                save()
                print(f"Validated {label} {variant}/{mode}: full outputs, guards and A/B/A PASS", flush=True)
        if not small:
            selected.append((extent, label, reference, cases, baseline))
    if args.validate_only:
        report["validation_complete"] = True
        save()
        print(f"Measurement mode validation PASS: {destination}")
        return
    timing = validation.copy()
    timing.pop("VK_INSTANCE_LAYERS", None)
    timing.pop("VK_LAYER_VALIDATE_SYNC", None)
    timing.pop("VK_LOADER_LAYERS_ENABLE", None)
    timing["VK_LOADER_LAYERS_DISABLE"] = "*"
    report["environment"] = {k: validation.get(k) for k in
                             ("VK_DRIVER_FILES", "VK_ICD_FILENAMES", "OGPU_VULKAN_LIBRARY", "VK_LAYER_PATH")}
    for round_index in range(3):
        for extent, label, reference, cases, baseline in selected:
            for mode in (MODES if round_index % 2 == 0 else tuple(reversed(MODES))):
                output = destination / label / f"run-{round_index}-{mode}"
                stdout, _ = invoke(build / "app-original", "--measure", mode, extent, reference, cases, output, timing)
                metadata, samples = parse_samples(stdout, mode)
                check_metadata(metadata, extent, mode, False)
                run.require(run.same_file(output / "normal-39-final.rgba", baseline / "normal-1-final.rgba"), "timed final B mismatch")
                row = dict(extent=extent, mode=mode, round=round_index, metadata=metadata, samples=samples,
                           statistics=summarize(samples))
                report["runs"].append(row)
                save()
                print(f"Measured {label} {mode} round {round_index}: median {row['statistics']['total_ms']['median']:.3f} ms", flush=True)
    for extent, label, reference, cases, baseline in selected:
        for mode in MODES:
            environment = validation.copy()
            environment["OGPU_TRACE_LOADER"] = validation.get("OGPU_VULKAN_LIBRARY", "libvulkan.so.1")
            environment["OGPU_VULKAN_LIBRARY"] = str(shim)
            output = destination / label / f"memory-{mode}"
            stdout, stderr = invoke(build / "app-original", "--measure", mode, extent, reference, cases, output, environment)
            metadata, _ = parse_samples(stdout, mode)
            check_metadata(metadata, extent, mode, False)
            memory = parse_memory(stderr)
            run.require(run.same_file(output / "normal-39-final.rgba", baseline / "normal-1-final.rgba"), "traced final B mismatch")
            report["memory"].append(dict(extent=extent, mode=mode, requested=metadata, native=memory))
            save()
            print(f"Memory {label} {mode}: peak {memory['peak_bytes']} bytes; {memory['allocations']} allocations; all freed", flush=True)
    report["complete"] = True
    save()
    print(f"OGPU measurement PASS: {destination}; no native Vulkan comparison")


if __name__ == "__main__":
    main()
