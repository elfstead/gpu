#!/usr/bin/env python3
"""Repeat the previously crashing odd-edge group; build with run.py --check first."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
sys.dont_write_bytecode = True
import benchmark as bench
import run


def main():
    build = run.ROOT / "target/learned-image"
    reference = build / "reference"
    import json
    manifest = json.loads((reference / "manifest.json").read_text())
    cases = [c for c in manifest["cases"] if c["group"] == "65x47-up"]
    run.require(len(cases) == 3, "generate the small reference first")
    run.require("VK_LAYER_KHRONOS_validation" in os.getenv("VK_INSTANCE_LAYERS", "")
                and os.getenv("VK_LAYER_VALIDATE_SYNC") == "1", "enable synchronization validation")
    destination = Path(tempfile.mkdtemp(prefix="display-edges-", dir=build))
    run.verified(reference / "weights.f32", manifest["weights_sha256"])
    for c in cases:
        for name, digest in c["sha256"].items():
            run.verified(reference / c["name"] / name, digest)
    baseline = None
    for variant in ("original", "mutated"):
        for repeat in range(10):
            output = destination / f"{variant}-{repeat}"
            output.mkdir()
            command = [str(build / f"app-{variant}"), str(reference / "weights.f32"),
                       "65", "47", "131", "95", str(output), *(str(reference / c["name"]) for c in cases)]
            p = subprocess.run(command, capture_output=True, text=True)
            (output / "stdout.txt").write_text(p.stdout)
            (output / "stderr.txt").write_text(p.stderr)
            run.require(p.returncode == 0 and "Validation Error:" not in p.stdout + p.stderr, f"failed: {output}")
            if baseline is None:
                bench.check_validation(output, reference, cases[:2])
                baseline = output
            else:
                for name in bench.output_files():
                    run.require(run.same_file(output / name, baseline / name), f"repeat differs: {output / name}")
            print(f"Display odd-edge {variant} repeat {repeat}: PASS", flush=True)
    print(f"20 fresh processes / 120 ordinary frames PASS: {destination}")


if __name__ == "__main__":
    main()
