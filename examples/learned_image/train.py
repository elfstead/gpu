#!/usr/bin/env python3
"""Fixed, stdlib-only Adam training. Never evaluates held-out acceptance scenes."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import sys
sys.dont_write_bytecode = True
import reference as ref

HERE = Path(__file__).resolve().parent
CONFIG = dict(steps=4096, batch=32, learning_rate=0.003, final_learning_rate=0.0003,
              decay_step=3072, beta1=0.9, beta2=0.999, epsilon=1e-8,
              initialization_seed=1729, sampling_seed=2718, training_size=[16, 16],
              training_seeds=list(ref.TRAIN_SEEDS), loss="unclipped residual MSE")


def source_hashes():
    return {name: hashlib.sha256((HERE / name).read_bytes()).hexdigest()
            for name in ("reference.py", "train.py")}


def train():
    # Training targets are clean-minus-noisy residuals. The final inference clamp
    # is not differentiated; this avoids saturating gradients during training.
    samples = []
    for seed in ref.TRAIN_SEEDS:
        clean, noisy = ref.scene(seed, 16, 16)
        for y in range(16):
            for x in range(16):
                samples.append(([v - 0.5 for v in ref.patch(noisy, 16, 16, x, y)],
                                clean[y * 16 + x] - noisy[y * 16 + x]))
    init = ref.Random(CONFIG["initialization_seed"])
    weights = [(init.uniform() - 0.5) * 0.5 for _ in range(72)] + [0.1] * 8 + [0.0] * 9
    first, second = [0.0] * 89, [0.0] * 89
    rng = ref.Random(CONFIG["sampling_seed"])
    for step in range(1, CONFIG["steps"] + 1):
        gradient = [0.0] * 89
        for _ in range(CONFIG["batch"]):
            data, target = samples[rng.word() % len(samples)]
            active = [max(0.0, sum(weights[c * 9 + k] * data[k] for k in range(9))
                          + weights[72 + c]) for c in range(8)]
            prediction = sum(weights[80 + c] * active[c] for c in range(8)) + weights[88]
            error = 2 * (prediction - target) / CONFIG["batch"]
            gradient[88] += error
            for c in range(8):
                gradient[80 + c] += error * active[c]
                if active[c] > 0:
                    derivative = error * weights[80 + c]
                    gradient[72 + c] += derivative
                    for k in range(9):
                        gradient[c * 9 + k] += derivative * data[k]
        rate = CONFIG["learning_rate"] if step <= CONFIG["decay_step"] else CONFIG["final_learning_rate"]
        for i, derivative in enumerate(gradient):
            first[i] = CONFIG["beta1"] * first[i] + (1 - CONFIG["beta1"]) * derivative
            second[i] = CONFIG["beta2"] * second[i] + (1 - CONFIG["beta2"]) * derivative * derivative
            m = first[i] / (1 - CONFIG["beta1"] ** step)
            v = second[i] / (1 - CONFIG["beta2"] ** step)
            weights[i] -= rate * m / (math.sqrt(v) + CONFIG["epsilon"])
    return [ref.f32(v) for v in weights]


def artifact(weights):
    return dict(schema=1, model="residual-3x3-1x8-relu-1x1-8x1-v1",
                license="MIT", sources=source_hashes(), training=CONFIG,
                training_arithmetic="Python binary64; final parameters rounded to binary32",
                weight_sha256=hashlib.sha256(ref.packed(weights)).hexdigest(), weights=weights)


def encoded(model):
    return json.dumps(model, indent=2, allow_nan=False) + "\n"


def load(path):
    model = json.loads(path.read_text())
    weights = model["weights"]
    ref.check_weights(weights)
    # Reject stale source/config/model metadata as well as modified parameters.
    if model != artifact(weights):
        raise ValueError("model provenance/hash does not match this recipe")
    return weights


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--output", type=Path, help="write an explicitly selected model file")
    mode.add_argument("--check", action="store_true", help="retrain and compare the frozen model without writing")
    args = parser.parse_args()
    result = encoded(artifact(train()))
    if args.check:
        if result != (HERE / "model.json").read_text():
            raise ValueError("retrained model differs; do not silently replace the frozen fixture")
        print("Training reproduction: byte-identical model PASS")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(result)
        print(f"Wrote trained model to {args.output}")


if __name__ == "__main__":
    main()
