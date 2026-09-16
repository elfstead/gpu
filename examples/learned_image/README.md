# Learned-image flagship

A project-owned, synthetic-data residual denoiser and scalar reference for the
[mixed ML/graphics application](../../docs/learned-image.md). Currently this is
the **CPU fixture checkpoint**, not an implemented GPU application.

```sh
python3 examples/learned_image/check.py
python3 examples/learned_image/check.py --retrain
```

Python standard library only; no downloads, NumPy, PyTorch or GPU needed. The
second command also repeats the fixed training run and requires a byte-identical
`model.json`. Recorded training environment: CPython 3.14.7 on Linux x86-64.
Bit-identical training across Python versions/platform math libraries is not
promised. Normal checks use the frozen weights and do not retrain or overwrite
them. An intentional new model requires `train.py --output <path>` and a new
acceptance record; do not replace the model just to make reproduction pass.

`reference.py` defines scenes, the FP64 scalar oracle over FP32 inputs/weights,
replicated padding, bilinear resize, affine palette and RGBA8 conversion.
`train.py` implements fixed-step Adam in binary64, then rounds all 89 parameters
to FP32. It trains on 32 scenes of 16x16 pixels, seeds 41–72, independently sampled
minibatches of 32 patches for 4,096 steps. The final checkpoint is used, with no
held-out selection. Architecture, noise, seed/configuration, source hashes and
FP32 weight hash are recorded in `model.json`. All data, code and weights are
project-authored and covered by the repository MIT license.

The checker runs analytical/rejection tests, verifies provenance and fixed
held-out quality gates, and exports raw files under
`target/learned-image/reference`. `manifest.json` specifies extents, grouping,
little-endian encodings and hashes. It includes eight quality cases plus 30
lifecycle cases: five input shapes, two resize extents and A/B/A frames per group.
FP32 input and weights are executable inputs; FP64 hidden, denoised and processed
values plus RGBA8 final pixels are diagnostic oracles. Never feed those outputs
to GPU inference. Clean targets are evaluation-only data.

`preview.ppm` shows clean / noisy / denoised for held-out seed 1005 using the
application palette. It is generated for inspection, not a separate acceptance
gate. `quality.json` contains all per-scene metrics. Generated files are not
versioned; their source recipe and the frozen learned model are.

Quality applies only to this synthetic piecewise-smooth/noise distribution.
Neither the small fixture sizes nor successful denoising establish performance,
photographic quality, GPU correctness or Metal graphics support.
