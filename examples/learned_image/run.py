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


def verified(path, digest):
    data = path.read_bytes()
    require(hashlib.sha256(data).hexdigest() == digest, f"fixture digest mismatch: {path}")
    return data


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
    args = parser.parse_args()
    os.chdir(ROOT)
    require(not os.getenv("CARGO_TARGET_DIR"), "leave CARGO_TARGET_DIR unset for this runner")
    command(sys.executable, "-B", str(HERE / "check.py"))
    command(sys.executable, "-B", str(HERE / "test_runner.py"))
    build = ROOT / "target/learned-image"
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
    outputs = {variant: execute_variant(build, library, executable, variant)
               for variant, executable in executables.items()}
    # Mutation changes mechanics only, not shader arithmetic or application policy.
    require(outputs["original"] == outputs["mutated"], "variant output sets differ")
    for path in outputs["original"]:
        require((build / "gpu/original" / path).read_bytes() == (build / "gpu/mutated" / path).read_bytes(),
                f"interface mutation changed results: {path}")
    print("Original/mutated interfaces: unchanged host source, all GPU outputs byte-identical PASS")


def execute_variant(build, library, executable, variant):
    environment = os.environ.copy()
    environment["LD_LIBRARY_PATH"] = str(library) + (os.pathsep + environment["LD_LIBRARY_PATH"]
                                                     if environment.get("LD_LIBRARY_PATH") else "")
    reference = build / "reference"
    manifest = json.loads((reference / "manifest.json").read_text())
    require(manifest["schema"] == 1 and manifest["byte_order"] == "little", "unsupported fixture format")
    verified(HERE / "model.json", manifest["model_sha256"])
    expected_weights = verified(reference / "weights.f32", manifest["weights_sha256"])
    groups = collections.defaultdict(list)
    for case in manifest["cases"]:
        # A quality scene is its own group; lifecycle scenes explicitly share.
        key = case["name"] if case["group"] == "quality" else case["group"]
        groups[key].append(case)
    max_error, max_ratio, max_pixel = 0.0, 0.0, 0
    results = []
    checked_outputs = []
    for name, cases in groups.items():
        output = build / "gpu" / variant / name
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
            expected = (oracle / "final.rgba").read_bytes()
            outputs = [(output / f"{mode}-{frame}-final.rgba").read_bytes() for mode in ("normal", "diagnostic")]
            require(outputs[0] == outputs[1], "diagnostic mode changed final pixels")
            max_pixel = max(max_pixel, compare_pixels(outputs[0], expected))
            for stage in ("hidden", "denoised", "processed"):
                actual = (output / f"diagnostic-{frame}-{stage}.f32").read_bytes()
                error, ratio = compare_float(actual, (oracle / f"{stage}.f64").read_bytes())
                max_error, max_ratio = max(max_error, error), max(max_ratio, ratio)
            require((output / f"diagnostic-{frame}-input.f32").read_bytes() == (oracle / "input.f32").read_bytes(),
                    "GPU changed input")
            require((output / f"diagnostic-{frame}-weights.f32").read_bytes() == expected_weights,
                    "GPU changed weights")
            checked_outputs.extend(Path(name) / f"{mode}-{frame}-final.rgba" for mode in ("normal", "diagnostic"))
            checked_outputs.extend(Path(name) / f"diagnostic-{frame}-{stage}.f32"
                                   for stage in ("input", "weights", "hidden", "denoised", "processed"))
        if len(cases) == 3:
            for mode in ("normal", "diagnostic"):
                a, b, again = [(output / f"{mode}-{frame}-final.rgba").read_bytes() for frame in range(3)]
                require(a == again and a != b, "A/B/A reuse did not distinguish/reproduce frames")
            for stage in ("hidden", "denoised", "processed"):
                require((output / f"diagnostic-0-{stage}.f32").read_bytes()
                        == (output / f"diagnostic-2-{stage}.f32").read_bytes(), "intermediate A reuse mismatch")
        print(f"Group {name}: independent oracles, guards, unchanged inputs/weights, reuse PASS", flush=True)
    summary = (f"Learned-image Vulkan {variant} PASS: {len(manifest['cases'])} cases in both normal/diagnostic modes; "
               f"max float error={max_error:.9g}, error/bound={max_ratio:.9g}, RGB code delta={max_pixel}; "
               "no intermediate CPU waits/reads or GPU representation copies")
    print(summary)
    (build / f"last-run-{variant}.txt").write_text("\n".join(results) + "\n" + summary + "\n")
    return checked_outputs


if __name__ == "__main__":
    main()
