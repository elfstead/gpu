# Consecutive draw-state encoding

Accepted 2026-10-02 at clean **`3df8349`**, against the
[first grouped timing baseline](graphics-range-timing-results.md) at `cee13c9`.
Retain the bounded backend correction; the public API and ABI remain unchanged.

## What changed

During a single recording, consecutive draws with identical raster identity,
index-buffer identity/range/format and copied root bytes bind graphics state only
once. Every non-draw step invalidates the remembered state. Zero-capacity ranges
emit nothing and do not install their requested state. No state survives a new
recording. Keys borrow retained steps; no additional allocation or reference-count
operation is needed. Root comparison does not inspect pointed-to memory.

Every nonempty native draw command still executes with its own record/count
descriptor and local draw identity. There is no draw fusion, count readback,
dependency change, new retention rule or driver-specific workaround.

## Correctness and native commands

The ordinary Rust suite passes 46 tests, including four state-key/empty/boundary
tests. Both local Vulkan drivers pass all 33 GPU tests; clippy and formatting pass.
The generated strict Radeon consumer passes all 540 frames. Fresh clean-source
matched range/reset/replay runs pass **72,000 Radeon and 4,608 llvmpipe frames**,
with full per-frame image/geometry/count/control checking and synchronization
validation. These are the same capacities, slots, strategies and budgets as the
[range reuse checkpoint](graphics-range-reuse-results.md).

At 512 separate draws per encoding, public graphics pipeline/root/index binding
counts fall from 512 each to one each, matching the native control. Compute still
adds one pipeline bind/root push. All 512 native draw commands remain. Trace
checks cover compiled replay commands as well as hot deltas. Public/native
barriers remain eight/six; allocation shapes and peak bytes remain equal, with
zero final live allocations. Private native command-memory bytes remain unknown.

The strict llvmpipe consumer still fails the same 12 counted→fixed configurations;
48 unaffected configurations pass. This optimization neither fixes nor waives the
[independently reproduced native driver failure](draw-count-followup.md).

## Matched timing

Same RX 5700 XT, shaders, target, harness and policies as the baseline: 257×193,
three rotated rounds, 216 processes, 100 warmups and 1,000 samples/process.
Correctness/source/runtime gates pass; validation/tracing are disabled for timing.
All final-slot byte checks and drains pass. Wall/frame remains GPU-copy-complete,
not full host-output consumption or isolated GPU duration.

| 512-record case | Slots | Old → new public/native wall ratio | New native/public host record+submit (µs) |
|---|---:|---:|---:|
| Separate calls, reset | 1 | 1.803 → 1.239 | 38.17 / 74.00 |
| Separate calls, replay | 1 | 1.066 → 1.006 | 12.43 / 12.60 |
| Separate calls, reset | 2 | 1.090 → 1.006 | 39.70 / 69.66 |
| Separate calls, replay | 2 | 1.087 → 1.005 | 12.43 / 12.42 |
| Fixed range, reset | 1 | 1.016 → 1.025 | 23.02 / 26.34 |
| GPU count, reset | 1 | 1.029 → 1.019 | 24.42 / 27.06 |

One-slot separate reset wall/frame falls from 0.3694 to 0.2508 ms public;
its contemporary native value is 0.2024 ms. Public median host work falls from
122.87 to 74.00 µs. Fixed/count wall ratios now span 0.989–1.056.
These are medians across three process summaries, not pooled independent samples.
Old/new runs are separate sessions, not randomized same-session A/B; no confidence
interval, universal tail parity or exact causal allocation of time is claimed.

The command traces establish removal of redundant backend work. The timing change
is consistent with a substantial benefit, while the remaining separate-reset host
gap is real evidence to keep investigating. Two-slot throughput does not erase
that host cost. Per-call validation, copied roots, retained objects and deferred
step encoding are candidates—not measured explanations. Direct encoding can avoid
deferred-step overhead, but cannot assume unchanged caller root bytes. The
[subsequent argument/state audit](argument-reuse-plan.md) separates that contract
obligation from implementation cost; neither the entire gap nor its absence is
established as inherent by this measurement.

## Next

Proceed to the predeclared useful-scale and labelled scope-break controls, then
complete scene installation. Keep the separate-call re-recording cost classified
as unresolved backend work; do not declare general Vulkan parity or full M4
completion. Further optimization should isolate preparation/encoding costs rather
than merge operations with different identity. No additional physical GPU or
Metal round trip is needed for these local steps.

## Receipts

[Radeon correctness](results/draw-state-2026-10-02/correctness-radv.json),
[software correctness](results/draw-state-2026-10-02/correctness-lvp.json),
[timing](results/draw-state-2026-10-02/timing.json), and
[36-pair old/new summary](results/draw-state-2026-10-02/summary.json).
Reports are normalized wrappers preserving all original fields and original byte
hash/path; raw sample/per-frame log hashes and paths remain in them. Runner,
GPU-suite and strict consumer logs are archived alongside. GPU-suite and generated
Radeon consumer logs are candidate development checks; the matched correctness
and timing reports explicitly record clean `3df8349` provenance.
