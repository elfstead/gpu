# Indexed draws: stronger native strategy correctness

Accepted 2026-09-30 at clean **`62f1ad7bbc2ae9f6ca2562eab4a37643d9dc2272`**.
This is a native-only correctness increment of the
[performance brief](indexed-depth-performance-plan.md), not timing or public API
acceptance. The runtime and ABI are unchanged.

## What the probe establishes

At capacities 1/64/512, compare three explicit Vulkan strategies:

1. N single-record indexed commands in one rendering scope, with stable binding.
2. One indexed command executing N contiguous indirect records.
3. One indexed command reading its active count from GPU memory, with capacity N.

Only this experimental executable queries/enables `multiDrawIndirect` and
`drawIndirectCount`; it checks `maxDrawIndirectCount >= 512`. All three strategies
enable the same features. Both tested drivers report a limit of 4,294,967,295.
Ordinary native controls and the public runtime keep their existing profile.

The counted operation reads its count during execution and limits it by the
declared maximum. See the official
[counted command](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdDrawIndexedIndirectCount2KHR.html)
and [count-range description](https://docs.vulkan.org/refpages/latest/refpages/source/VkDrawIndirectCount2InfoKHR.html).
The probe uses the pinned Vulkan headers and existing address-command baseline.

Compute writes vertices, UINT32 indices, N 20-byte draw records and the count
before rasterization in the **same submission**. Host input requests phase/count;
there is no intermediate readback, but this is not a GPU culling algorithm.
Eight frames request `N, 0, 1, N/2, N+7, N, 0, 1`, with alternating phase.
The single/multi controls zero inactive records. The counted control leaves
**every record nonempty**: an empty image therefore cannot be explained by
record masking instead of count consumption.

Only the last record draws the moving near green surface; earlier records draw
the far red surface. Capacity one is far-only. This distinguishes partial and
full execution even at N=512. It does not identify every intermediate count
through pixels; complete record/count readback and command-argument tracing
complement the image check. Above-capacity input exercises the bounded command,
not an out-of-range draw-record allocation.

## Accepted matrix and budgets

| Driver / extent | Configurations | Checked frontier frames | Original native regression frames |
|---|---:|---:|---:|
| Radeon / 257×193 | 9 | 72 | 30 |
| Radeon / 1280×720 | 9 | 72 | 30 |
| llvmpipe / 257×193 | 9 | 72 | 30 |

All **216 frontier frames** pass Vulkan/synchronization validation. The runner
rechecks the accepted reference hashes and recomputes its analytic oracle before
comparing every output-image byte. It independently checks all generated
vertices/indices/records/counts/guards, and the consumer checks the full immutable
control input after execution. Vertex/fragment binaries are byte-identical to the
accepted scene's artifacts. The refactored ordinary native scene still matches
all ten modes and A/B/A frames, without enabling the experimental features.

Native tracing confirms eight rendering scopes/submissions per configuration:
single records encode `8×N` draw calls; multi-record execution encodes eight calls
with `8×N` total declared records; counted execution encodes eight counted calls
with `8×N` total declared capacity. These are **recorded command arguments**, not
GPU hardware counters. One additional submission initializes resources.

All strategies have nine identical tracked allocations for a given extent/N,
with the same allocation size/type multiset and peak bytes. All allocations and
command pools are freed; no successful path uses queue-idle.

| N | Requested bytes, small / 720p | Radeon peak bytes, small / 720p | llvmpipe peak bytes, small |
|---|---|---|---:|
| 1 | 795,408 / 14,747,392 | 1,082,664 / 15,394,584 | 825,096 |
| 64 | 799,084 / 14,751,068 | 1,086,360 / 15,398,280 | 828,772 |
| 512 | 825,964 / 14,777,948 | 1,113,240 / 15,425,160 | 855,652 |

Requested bytes include attachments, geometry, records/count, input, upload and
readback buffers. Traced bytes are their native memory allocations, not total
driver/executable/command memory. This probe uses **one serial slot with fresh
per-frame command storage**, not the planned stable reset/replay timing policy.

## Verification and retained evidence

- [Radeon report and raw process logs](results/graphics-indexed-frontier-radv-2026-09-30/report.json),
  SHA-256 `af14d41f0f831d461a4551a216f1a7e36eabe0ae9c7bbadba74c25ba392dc859`.
- [llvmpipe report and raw process logs](results/graphics-indexed-frontier-lvp-2026-09-30/report.json),
  SHA-256 `f0c8c8118c9ac7cd2fd1fe442d3a5c4dcb8c7185708a4a18e3f693a6459a8300`.
- [Shared-helper regression receipt](results/graphics-indexed-frontier-validation-2026-09-30/regression-receipt.json),
  with driver logs, sanitizer logs and analyzer output alongside it.

Frontier reports/logs are byte-identical exports. Source, executable, shader and
output/log hashes were verified. Full local directories are
`target/graphics-scene/frontier-7b3kdbpb` and `frontier-h467igxt`; image/geometry
dumps remain local. Reproduce using the [README](../examples/graphics_scene/README.md#native-indexed-frontier).

The existing reuse runner additionally passes its bounded eight-frame preflight
on both drivers: UINT16/UINT32 × four preservation modes × one/two slots ×
reset/replay × native/public. That adds **1,024 reuse frames and 240 serial
regression frames**. This is regression evidence, not a rerun of the accepted
1,000-frame stress matrix. Its complete reports/process logs remain local at
`reuse-gihtj3fp` and `reuse-0sx085nx`; the receipt retains hashes and verified totals.

C builds deny warnings. Ten Python tests pass across frontier/reuse/runner/oracle
suites, including corrupted native strategy/feature/frame/memory reports. Clang
analysis of `frontier.c` reports no diagnostics (the Nix wrapper's unused linker
argument is suppressed, not an analyzer warning). ASan/UBSan pass eight Radeon
512-record counted frames with exact output agreement; LeakSanitizer is disabled.
No runtime implementation changed; the full Rust/GPU contract suite was not
rerun for this native-consumer increment.

## Decision and remaining gate

The stronger native paths exist and work on the available implementations;
missing baseline feature enablement was not hardware absence. They must remain
in the comparison. **No fundamental performance approval follows.**

The public surface still expresses one record per call and no count-buffer
operation. This scene's masking alternative changes the GPU producer and executes
a capacity-sized command sequence. It establishes equivalent final pixels here,
not preservation of the counted strategy, arbitrary producer semantics or cost.
The shader does not consume draw ID; fusion preserving shader-visible identity
is still untested. Treat explicit indirect ranges/stride and a retained GPU count
word as a concrete better API alternative to evaluate, not as compatibility debt
to work around.

Next carry these controls into the planned matched public/native grouped
reset/replay measurements at one/two slots. Include actual binding/root/scope/
dependency traces, the labelled per-draw-scope control and draw-identity analysis.
Decide the range/count contract from preserved strategies and semantics, not only
the timing ratio of this redundant-geometry scene. Generated/installed scene
handoff and the remaining mip/view/blend slices follow.
