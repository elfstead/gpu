#!/usr/bin/env python3
"""Reproduce the fixed two-slot arena/replay handoff using only the copied SDK example."""
import argparse
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
EXTENT = [65, 47, 131, 95]
GATES = ("Streaming full-frame pixels/readback guards PASS; all slots drained",
         "Streaming final input/weight integrity and all intermediate guards PASS",
         "Reuse HOST padding and all slot generations retired PASS")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def records(text, prefix):
    return [json.loads(line[len(prefix):]) for line in text.splitlines() if line.startswith(prefix)]


def single(text, prefix):
    found = records(text, prefix)
    require(len(found) == 1, f"missing/duplicate {prefix}")
    return found[0]


def check(stdout, stderr, allocation, policy, slots, frames):
    require("Validation Error:" not in stdout + stderr, "Vulkan validation error")
    require(all(gate in stdout for gate in GATES), "missing correctness/drain gate")
    result = single(stdout, "STREAM ")
    expected = dict(policy=policy, slots=slots, extent=EXTENT, frames=frames, warmups=0, validation=True)
    require(all(result.get(k) == v for k, v in expected.items()), "wrong execution policy")
    require(result.get("max_rgb_delta") in (0, 1), "pixel tolerance failed")
    require(all(math.isfinite(result[k]) and result[k] > 0 for k in ("setup_ms", "wall_ms")), "invalid clocks")
    require(not records(stdout, "FRAME "), "timing mode is not handoff validation")
    device = single(stdout, "DEVICE ")
    memory = single(stdout, "REUSE_MEMORY ")
    require(memory["allocation"] == allocation and memory["buffer_allocations"] == (4 if allocation == "arena" else 1+7*slots),
            "wrong backing policy")
    ranges = records(stdout, "REUSE_RANGE ")
    require(len(ranges) == 7*slots, "incomplete ranges")
    ends = {}
    for index, row in enumerate(ranges):
        slot, resource = divmod(index, 7)
        backing = (0 if resource < 5 else resource-4) if allocation == "arena" else index
        require((row["slot"], row["resource"], row["backing"], row["host"]) ==
                (slot, resource, backing, resource >= 5), "wrong range ownership")
        require(row["atom"] > 0 and row["size"] > 0 and row["offset"] >= ends.get(backing, 0)
                and row["offset"] % row["atom"] == row["end"] % row["atom"] == 0
                and row["offset"] + row["size"] <= row["end"], "overlapping/invalid range")
        ends[backing] = row["end"]
    return dict(result=result, device=device, memory=memory, ranges=ranges)


def fixtures(destination):
    spec = importlib.util.spec_from_file_location("reference", HERE / "reference.py")
    ref = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(ref)
    model = json.loads((HERE / "model.json").read_text())
    weights = model["weights"]
    ref.check_weights(weights)
    packed = ref.packed(weights)
    require(hashlib.sha256(packed).hexdigest() == model["weight_sha256"], "model weight hash mismatch")
    (destination / "weights.f32").write_bytes(packed)
    for label, seed in (("a", 2001), ("b", 2002)):
        _, noisy = ref.scene(seed, *EXTENT[:2])
        _, gray = ref.infer(weights, noisy, *EXTENT[:2])
        _, pixels = ref.render_reference(gray, *EXTENT)
        (destination / f"input-{label}.f32").write_bytes(ref.packed(noisy))
        (destination / f"reference-{label}.rgba").write_bytes(pixels)
    paths = [destination / n for n in ("weights.f32", "input-a.f32", "input-b.f32", "reference-a.rgba", "reference-b.rgba")]
    return paths


def execute(command, environment, destination, label):
    # Files retain partial output even on a timeout/crash; never accept a timeout
    # as a completed drain. subprocess kills/reaps only this child on timeout.
    out, err = destination / f"{label}.stdout", destination / f"{label}.stderr"
    with out.open("w") as stdout, err.open("w") as stderr:
        result = subprocess.run(command, cwd=HERE, env=environment, stdout=stdout, stderr=stderr, timeout=300)
    require(result.returncode == 0, f"{label} exited {result.returncode}; inspect {err}")
    return out.read_text(), err.read_text()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--frames", type=int, default=1000)
    args = parser.parse_args()
    if not 4 <= args.frames <= 10000:
        parser.error("frames must be between 4 and 10000")
    destination = Path(tempfile.mkdtemp(prefix="run-", dir=HERE))
    print(f"Handoff artifacts: {destination}", flush=True)
    report = dict(schema=1, complete=False, scope="installed public C consumer correctness; not timing or external adoption",
                  frames=args.frames, runs=[], environment={k: os.getenv(k) for k in
                  ("VK_DRIVER_FILES", "VK_INSTANCE_LAYERS", "VK_LAYER_VALIDATE_SYNC", "OGPU_VULKAN_LIBRARY")})

    def save():
        (destination / "report.json").write_text(json.dumps(report, indent=2, allow_nan=False)+"\n")

    save()
    try:
        pkg = os.getenv("PKG_CONFIG", "pkg-config")
        def variable(name):
            return subprocess.check_output([pkg, f"--variable={name}", "ogpu"], text=True).strip()
        library = (Path(variable("libdir")) / "libogpu.so").resolve()
        linked = subprocess.check_output(["ldd", str(HERE / "reuse")], text=True)
        (destination / "ldd.txt").write_text(linked)
        selected = next((line.split("=>", 1)[1].rsplit(" (", 1)[0].strip() for line in linked.splitlines()
                         if line.strip().startswith("libogpu.so =>")), None)
        require(selected is not None and Path(selected).resolve() == library, "linked runtime differs from pkg-config SDK")
        report["sdk"] = dict(revision=variable("ogpu_revision"), abi=int(variable("ogpu_abi")),
                             runtime_sha256=digest(library), runtime_path=str(library))
        report["executable_sha256"] = digest(HERE / "reuse")
        sources = [p for p in HERE.rglob("*") if p.is_file() and not any(part.startswith("run-") for part in p.relative_to(HERE).parts)
                   and p.suffix in (".c", ".h", ".py", ".json", ".md")]
        report["source_sha256"] = {str(p.relative_to(HERE)): digest(p) for p in sorted(sources)}
        paths = fixtures(destination)
        report["fixture_sha256"] = {p.name: digest(p) for p in paths}
        environment = os.environ.copy()
        environment["OGPU_STREAM_SERIAL_A"] = str(destination / "serial-a.rgba")
        environment["OGPU_STREAM_SERIAL_B"] = str(destination / "serial-b.rgba")
        save()
        for allocation, policy, slots, frames, mode in (("dedicated", "ogpu", 1, 12, "write"),
                                                       ("arena", "compiled", 2, args.frames, "check")):
            label = "serial" if mode == "write" else "candidate"
            environment["OGPU_STREAM_ALLOCATION"] = allocation
            environment["OGPU_STREAM_SERIAL_MODE"] = mode
            command = [str(HERE / "reuse"), policy, str(slots), *map(str, EXTENT), str(frames), "validate", *map(str, paths)]
            stdout, stderr = execute(command, environment, destination, label)
            record = check(stdout, stderr, allocation, policy, slots, frames)
            record["log_sha256"] = {f"{label}.{suffix}": digest(destination / f"{label}.{suffix}") for suffix in ("stdout", "stderr")}
            if report["runs"]:
                require(record["device"] == report["runs"][0]["device"], "device changed between controls")
            report["runs"].append(record)
            save()
        report["serial_sha256"] = {f"serial-{label}.rgba": digest(destination / f"serial-{label}.rgba") for label in ("a", "b")}
        report["complete"] = True
        save()
    except Exception as error:
        report["error"] = str(error)
        save()
        raise
    print(f"Installed two-slot arena/replay PASS: {args.frames} checked frames; report: {destination / 'report.json'}")


if __name__ == "__main__":
    main()
