#!/usr/bin/env python3
"""Build and check the complete Vulkan learned-image application."""
import argparse
import collections
import hashlib
import json
import math
import os
from pathlib import Path
import shlex
import struct
import subprocess
import sys
sys.dont_write_bytecode = True
import generate_interfaces

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def command(*args, **kwargs):
    try:
        return subprocess.run(args, check=True, **kwargs)
    except subprocess.CalledProcessError as error:
        if error.stdout:
            print(error.stdout, end="", file=sys.stderr)
        if error.stderr:
            print(error.stderr, end="", file=sys.stderr)
        raise


def digest_file(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def verified(path, digest):
    require(digest_file(path) == digest, f"fixture digest mismatch: {path}")


def same_file(a, b):
    if a.stat().st_size != b.stat().st_size:
        return False
    with a.open("rb") as left, b.open("rb") as right:
        while block := left.read(256 * 1024):
            if block != right.read(len(block)):
                return False
    return True


def compare_files(actual, expected, floating=False):
    size, reference_size = actual.stat().st_size, expected.stat().st_size
    require(size > 0 and size % 4 == 0 and reference_size == size * (2 if floating else 1),
            f"file extent mismatch: {actual}")
    maximum, ratio = 0.0, 0.0
    with actual.open("rb") as gpu, expected.open("rb") as cpu:
        offset = 0
        while block := gpu.read(256 * 1024):
            reference = cpu.read(len(block) * (2 if floating else 1))
            try:
                if floating:
                    error, bound = compare_float(block, reference)
                    maximum, ratio = max(maximum, error), max(ratio, bound)
                else:
                    maximum = max(maximum, compare_pixels(block, reference))
            except ValueError as error:
                raise ValueError(f"{actual}, chunk starting at byte {offset}: {error}") from error
            offset += len(block)
    return (maximum, ratio) if floating else int(maximum)


def compare_float(actual, expected):
    require(len(expected) % 8 == 0 and len(actual) * 2 == len(expected), "float extent mismatch")
    count = len(actual) // 4
    require(count > 0, "empty float comparison")
    observed = struct.unpack(f"<{count}f", actual)
    reference = struct.unpack(f"<{count}d", expected)
    maximum, ratio = 0.0, 0.0
    for index, (value, correct) in enumerate(zip(observed, reference)):
        require(math.isfinite(value) and math.isfinite(correct), f"nonfinite float at {index}")
        delta = abs(value - correct)
        bound = 2e-5 + 2e-5 * abs(correct)
        require(delta <= bound, f"float mismatch at {index}: {value} vs {correct}, bound {bound}")
        maximum, ratio = max(maximum, delta), max(ratio, delta / bound)
    return maximum, ratio


def compare_pixels(actual, expected):
    require(len(actual) == len(expected) and len(actual) > 0 and len(actual) % 4 == 0,
            "RGBA extent mismatch")
    maximum = 0
    for index, (value, correct) in enumerate(zip(actual, expected)):
        if index % 4 == 3:
            require(value == correct == 255, f"alpha mismatch at {index // 4}")
        else:
            delta = abs(value - correct)
            require(delta <= 1, f"RGB mismatch at byte {index}: {value} vs {correct}")
            maximum = max(maximum, delta)
    return maximum


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="CPU checks and compile/validate only; no GPU")
    parser.add_argument("--scale", action="store_true", help="also check 720p/1080p A/B/A groups (large files; Radeon acceptance)")
    args = parser.parse_args()
    os.chdir(ROOT)
    require(not os.getenv("CARGO_TARGET_DIR"), "leave CARGO_TARGET_DIR unset for this runner")
    command(sys.executable, "-B", str(HERE / "check.py"))
    command(sys.executable, "-B", str(HERE / "test_runner.py"))
    build = ROOT / "target/learned-image"
    build.mkdir(parents=True, exist_ok=True)
    compiler = shlex.split(os.getenv("CC", "cc"))
    command(*compiler, "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror",
            str(HERE / "test_extent.c"), "-o", str(build / "test-extent"))
    command(str(build / "test-extent"))
    command(*compiler, "-std=c11", "-O2", "-ffp-contract=off", "-fno-fast-math",
            "-Wall", "-Wextra", "-Werror", str(HERE / "reference_stream.c"), "-lm",
            "-o", str(build / "reference-stream"))
    check_streaming_reference(build)
    if args.scale:
        export_scale_reference(build)
    interfaces = generate_interfaces.build_interfaces(args.check)
    command(sys.executable, "-B", str(HERE / "test_interfaces.py"))
    command(os.getenv("CARGO", "cargo"), "build", "--locked", "--release", "-p", "ogpu")
    library = ROOT / "target/release"
    executables = {}
    for variant, headers in interfaces.items():
        executable = build / f"app-{variant}"
        command(*shlex.split(os.getenv("CC", "cc")), "-std=c11", "-O2", "-DNDEBUG",
                "-Wall", "-Wextra", "-Werror", "-I", str(headers), "-I", "include", str(HERE / "app.c"),
                "-L", str(library), f"-Wl,-rpath,{library}", "-logpu", "-o", str(executable))
        executables[variant] = executable
    if args.check:
        print("Learned-image generated/mutated CPU/build/SPIR-V checks PASS; no GPU execution")
        return
    for suite in (("small", "scale") if args.scale else ("small",)):
        outputs = {variant: execute_variant(build, library, executable, variant, suite)
                   for variant, executable in executables.items()}
        # Mutation changes mechanics only, not shader arithmetic or application policy.
        require(outputs["original"] == outputs["mutated"], "variant output sets differ")
        for path in outputs["original"]:
            require(same_file(build / "gpu" / suite / "original" / path,
                              build / "gpu" / suite / "mutated" / path),
                    f"interface mutation changed results: {path}")
        print(f"{suite}: original/mutated interfaces, unchanged host source, all GPU outputs byte-identical PASS", flush=True)


def check_streaming_reference(build):
    reference = build / "reference"
    manifest = json.loads((reference / "manifest.json").read_text())
    for case in manifest["cases"]:
        output = build / "reference-crosscheck" / case["name"]
        output.mkdir(parents=True, exist_ok=True)
        command(str(build / "reference-stream"), str(reference / "weights.f32"), str(output),
                *(str(v) for v in case["input_size"] + case["output_size"]), str(case["seed"]),
                capture_output=True, text=True)
        for name in case["sha256"]:
            require(same_file(output / name, reference / case["name"] / name),
                    f"C/Python reference disagreement: {case['name']}/{name}")
    print(f"Streaming C FP64 oracle: all files byte-identical to {len(manifest['cases'])} Python cases PASS", flush=True)


def export_scale_reference(build):
    directory = build / "reference-scale"
    directory.mkdir(parents=True, exist_ok=True)
    original = json.loads((build / "reference/manifest.json").read_text())
    weights = (build / "reference/weights.f32").read_bytes()
    (directory / "weights.f32").write_bytes(weights)
    cases = []
    files = ("clean.f32", "input.f32", "hidden.f64", "denoised.f64", "processed.f64", "final.rgba")
    for w, h, ow, oh in ((1280, 720, 2560, 1440), (1920, 1080, 960, 540)):
        group = f"{w}x{h}-to-{ow}x{oh}"
        pair = []
        for seed in (2001, 2002):
            name = f"{group}-{seed}"
            output = directory / name
            output.mkdir(parents=True, exist_ok=True)
            command(str(build / "reference-stream"), str(directory / "weights.f32"), str(output),
                    *(str(v) for v in (w, h, ow, oh, seed)))
            pair.append(dict(name=name, group=group, seed=seed, input_size=[w, h], output_size=[ow, oh],
                             sha256={name: digest_file(output / name) for name in files}))
        cases.extend((pair[0], pair[1], pair[0]))
    manifest = dict(original, cases=cases)
    (directory / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print("Exported six video-scale A/B/A cases; full FP64 oracles, no sampled comparisons", flush=True)


def execute_variant(build, library, executable, variant, suite="small"):
    environment = os.environ.copy()
    environment["LD_LIBRARY_PATH"] = str(library) + (os.pathsep + environment["LD_LIBRARY_PATH"]
                                                     if environment.get("LD_LIBRARY_PATH") else "")
    reference = build / ("reference-scale" if suite == "scale" else "reference")
    manifest = json.loads((reference / "manifest.json").read_text())
    require(manifest["schema"] == 1 and manifest["byte_order"] == "little", "unsupported fixture format")
    verified(HERE / "model.json", manifest["model_sha256"])
    verified(reference / "weights.f32", manifest["weights_sha256"])
    groups = collections.defaultdict(list)
    for case in manifest["cases"]:
        # A quality scene is its own group; lifecycle scenes explicitly share.
        key = case["name"] if case["group"] == "quality" else case["group"]
        groups[key].append(case)
    max_error, max_ratio, max_pixel = 0.0, 0.0, 0
    results = []
    checked_outputs = []
    for name, cases in groups.items():
        output = build / "gpu" / suite / variant / name
        output.mkdir(parents=True, exist_ok=True)
        first = cases[0]
        require(all(c["input_size"] == first["input_size"] and c["output_size"] == first["output_size"]
                    for c in cases), "reuse group has inconsistent extents")
        # Verify the source bytes before they are uploaded, not only after execution.
        for case in cases:
            for filename, digest in case["sha256"].items():
                verified(reference / case["name"] / filename, digest)
        process = command(str(executable), str(reference / "weights.f32"),
                          *(str(v) for v in first["input_size"] + first["output_size"]), str(output),
                          *(str(reference / c["name"]) for c in cases),
                          env=environment, capture_output=True, text=True)
        print(process.stdout, end="", flush=True)
        print(process.stderr, end="", file=sys.stderr, flush=True)
        require("Validation Error:" not in process.stdout + process.stderr, "Vulkan validation error")
        results.append(process.stdout + process.stderr)
        for frame, case in enumerate(cases):
            oracle = reference / case["name"]
            outputs = [output / f"{mode}-{frame}-final.rgba" for mode in ("normal", "diagnostic")]
            require(same_file(*outputs), "diagnostic mode changed final pixels")
            max_pixel = max(max_pixel, compare_files(outputs[0], oracle / "final.rgba"))
            for stage in ("hidden", "denoised", "processed"):
                actual = output / f"diagnostic-{frame}-{stage}.f32"
                error, ratio = compare_files(actual, oracle / f"{stage}.f64", floating=True)
                max_error, max_ratio = max(max_error, error), max(max_ratio, ratio)
            require(same_file(output / f"diagnostic-{frame}-input.f32", oracle / "input.f32"),
                    "GPU changed input")
            require(same_file(output / f"diagnostic-{frame}-weights.f32", reference / "weights.f32"),
                    "GPU changed weights")
            checked_outputs.extend(Path(name) / f"{mode}-{frame}-final.rgba" for mode in ("normal", "diagnostic"))
            checked_outputs.extend(Path(name) / f"diagnostic-{frame}-{stage}.f32"
                                   for stage in ("input", "weights", "hidden", "denoised", "processed"))
        if len(cases) == 3:
            for mode in ("normal", "diagnostic"):
                a, b, again = [output / f"{mode}-{frame}-final.rgba" for frame in range(3)]
                require(same_file(a, again) and not same_file(a, b), "A/B/A reuse did not distinguish/reproduce frames")
            for stage in ("hidden", "denoised", "processed"):
                require(same_file(output / f"diagnostic-0-{stage}.f32",
                                  output / f"diagnostic-2-{stage}.f32"), "intermediate A reuse mismatch")
        print(f"Group {name}: independent oracles, guards, unchanged inputs/weights, reuse PASS", flush=True)
    summary = (f"Learned-image Vulkan {suite}/{variant} PASS: {len(manifest['cases'])} cases in both normal/diagnostic modes; "
               f"max float error={max_error:.9g}, error/bound={max_ratio:.9g}, RGB code delta={max_pixel}; "
               "no intermediate CPU waits/reads or GPU representation copies")
    print(summary)
    (build / f"last-run-{suite}-{variant}.txt").write_text("\n".join(results) + "\n" + summary + "\n")
    return checked_outputs


if __name__ == "__main__":
    main()
