#!/usr/bin/env python3
"""Check the frozen CPU fixture and export GPU-consumable reference cases."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
sys.dont_write_bytecode = True
import reference as ref
import train

ROOT = train.HERE.parents[1]


def export(weights, directory):
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "weights.f32").write_bytes(ref.packed(weights))
    cases = []
    # Same extents and allocations can be reused for A/B/A. Quality scenes also
    # have GPU oracles; the lifecycle scenes alone are not quality evidence.
    selected = [(f"quality-{seed}", seed, w, h, 2 * w + 1, 2 * h + 1, "quality")
                for seed, w, h in ref.QUALITY_CASES]
    for width, height in ref.EDGE_SHAPES:
        for label, ow, oh in (("up", 2 * width + 1, 2 * height + 1),
                              ("down", max(1, width // 2), max(1, height // 2))):
            group = f"{width}x{height}-{label}"
            selected.extend((f"{group}-{frame}", seed, width, height, ow, oh, group)
                            for frame, seed in enumerate((2001, 2002, 2001)))
    for name, seed, width, height, ow, oh, group in selected:
        clean, noisy = ref.scene(seed, width, height)
        hidden, denoised = ref.infer(weights, noisy, width, height)
        rgba, pixels = ref.render_reference(denoised, width, height, ow, oh)
        location = directory / name
        location.mkdir(exist_ok=True)
        files = {"clean.f32": ref.packed(clean), "input.f32": ref.packed(noisy),
                 "hidden.f64": struct.pack(f"<{len(hidden)}d", *hidden),
                 "denoised.f64": struct.pack(f"<{len(denoised)}d", *denoised),
                 "processed.f64": struct.pack(f"<{len(rgba)}d", *rgba),
                 "final.rgba": pixels}
        for filename, content in files.items():
            (location / filename).write_bytes(content)
        cases.append(dict(name=name, group=group, seed=seed, input_size=[width, height],
                          output_size=[ow, oh], sha256={filename: hashlib.sha256(data).hexdigest()
                                                     for filename, data in files.items()}))
    manifest = dict(schema=1, byte_order="little", reference_arithmetic="binary64",
                    model_sha256=hashlib.sha256((train.HERE / "model.json").read_bytes()).hexdigest(),
                    weights_sha256=hashlib.sha256(ref.packed(weights)).hexdigest(), cases=cases)
    (directory / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    # Viewable contact sheet: clean | noisy | denoised, one held-out scene.
    seed, width, height = ref.QUALITY_CASES[4]
    clean, noisy = ref.scene(seed, width, height)
    _, denoised = ref.infer(weights, noisy, width, height)
    panels = [ref.render_reference(v, width, height, width * 4, height * 4)[1]
              for v in (clean, noisy, denoised)]
    rgb = bytearray()
    for y in range(height * 4):
        for panel in panels:
            for x in range(width * 4):
                start = (y * width * 4 + x) * 4
                rgb.extend(panel[start:start + 3])
    (directory / "preview.ppm").write_bytes(f"P6\n{width * 12} {height * 4}\n255\n".encode() + rgb)
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--retrain", action="store_true", help="also reproduce trained model byte-for-byte")
    args = parser.parse_args()
    subprocess.run([sys.executable, "-B", str(train.HERE / "test_reference.py")], check=True)
    if args.retrain:
        subprocess.run([sys.executable, "-B", str(train.HERE / "train.py"), "--check"], check=True)
    weights = train.load(train.HERE / "model.json")
    report = ref.quality(weights)
    if not report["passed"]:
        raise ValueError("frozen quality gate failed")
    directory = ROOT / "target/learned-image/reference"
    manifest = export(weights, directory)
    (directory / "quality.json").write_text(json.dumps(report, indent=2) + "\n")
    for row in report["cases"]:
        print(f"seed {row['seed']}, {row['width']}x{row['height']}: "
              f"MSE noisy={row['noisy_mse']:.8f}, box={row['box_mse']:.8f}, CNN={row['cnn_mse']:.8f}")
    print(f"Pooled {report['pixels']} pixels: gain over noisy {report['gain_db']:.4f} dB, "
          f"over box {report['versus_box_db']:.4f} dB; quality PASS")
    print(f"Exported {len(manifest['cases'])} CPU cases to {directory}")
    print("CPU fixture PASS; this command does not execute or accept GPU work")


if __name__ == "__main__":
    main()
