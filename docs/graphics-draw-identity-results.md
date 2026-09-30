# Native draw identity: semantic decision evidence

Accepted 2026-09-30 at clean **`e6baf7bf5e4aed7fd5515d4fed8d23cc07b5514f`**.
The [native frontier](graphics-indexed-frontier-results.md) now has an identity-
sensitive variant. This closes its bounded draw-identity question, not public API
implementation or timing acceptance. Runtime/header remain ABI 18.

## Result

**Single-record commands and one multi-record command are observably different
programs when the vertex shader reads draw identity.** On both Radeon and
llvmpipe, the variant produces the expected different pixels for full-capacity
N=64/512 cases, while multi-record and counted execution agree. The experiment
requires these differences; they are not tolerated correctness failures.

This matches native [DrawIndex semantics](https://docs.vulkan.org/refpages/latest/refpages/source/DrawIndex.html).
Slang's [SPIR-V semantic mapping](https://docs.shader-slang.org/en/latest/external/slang/docs/user-guide/a2-01-spirv-target-specific.html)
maps `SV_DrawIndex` to that builtin. The experimental device alone queries/enables
`shaderDrawParameters`. The runner requires the compiled vertex artifact to
contain both `DrawParameters` capability and `BuiltIn DrawIndex`; ordinary vertex
artifacts contain neither. There is no shader rewrite, added root field or
record-level identity buffer.

Crucially, **ABI 18 does not enable shader draw parameters**. This is a native
semantic choice to preserve in the expanded profile, not a counterexample using
a currently supported public shader or evidence that every existing call pair
is unsafe to fuse. The GPU count remains a separate expressibility gap.

## Probe and accepted matrix

Keep the frontier's geometry, records, count generation, memory budgets, eight
input frames, dependencies and actual native-command tracing. Change only the
vertex/fragment programs and explicitly enabled draw-parameter feature. Encode
`2 * (draw_index + 1) + surface` in the red/green bytes and set blue/alpha to 255;
clear pixels stay black/alpha255. Surface is zero for the far quad and one for
the last-record near quad. Depth is unchanged.

| Capacity | Visible near identity, separate single calls | Visible near identity, multi/count |
|---|---:|---:|
| 1 | No near record | No near record |
| 64 | 0 | 63 |
| 512 | 0 | 511 |

The near surface executes only at full capacity; partial/empty outputs therefore
remain equal across strategies in this fixture. Each driver checks nine
configurations × eight frames at 257×193, for **144 identity frames total**.
There are six required single/multi image inequalities per driver (twelve across
both), and all 24 multi/count frame pairs per driver are exactly equal.

The oracle recolors the independently analytically checked reference coverage,
preserving all depth/guard bytes. Every output byte, GPU-generated record/count,
vertex/index and guard is checked. Native-call counts and allocation size/type
multisets still match the intended strategies and budgets. All nine allocations
per case are freed and command pools balanced, without successful queue-idle.
Validation and synchronization validation pass on both drivers.

The ordinary identity-insensitive frontier is rerun at the same clean revision:
another **144 frames**, with its original shaders and no draw-parameter feature.
All four runners also rerun the original ten-mode native scene, adding **120
serial regression frames**. No 720p or long reuse stress rerun is claimed here.

## Evidence

| Run | Local directory under `target/graphics-scene` | Report SHA-256 |
|---|---|---|
| [Radeon identity](results/graphics-draw-identity-radv-2026-09-30/report.json) | `frontier-0jz8pbv_` | `cbdcc8905d446a163906e72eb71cd14513589cb58f38391939980318f79f001e` |
| [llvmpipe identity](results/graphics-draw-identity-lvp-2026-09-30/report.json) | `frontier-bb9rvxa5` | `574abd12c72cfd4b3e7321c6fc4ea2d5e3e537d2cfa3a1fc1ef374ee04339970` |
| [Radeon ordinary regression](results/graphics-draw-identity-radv-regression-2026-09-30/report.json) | `frontier-y7jujuff` | `d9837e19d45294b7c11ed46565d91f22f548bdd5f35e75f2548e6ba7ed96d87e` |
| [llvmpipe ordinary regression](results/graphics-draw-identity-lvp-regression-2026-09-30/report.json) | `frontier-uxenel8f` | `dc45192b9af02243d0aea8c42bb20000efabdb5a1b9e61b8592aba5f1902d479` |

Reports and all native process stdout/stderr are retained beside those links;
exports are byte-identical. Source, executable, shader, image/geometry and process
log hashes were verified. Full image/geometry dumps remain in the local directories.
Reproduce with `frontier.py --identity --reference32 <accepted-same-driver-report>`;
omit `--identity` for the ordinary regression. See the consumer README for the
compiler/ICD/validation environment.

Twelve Python tests pass across frontier/reuse/runner/oracle, including rejection
of missing/unexpected identity enablement and checked RGBA8 recoloring with
unchanged guards/depth. Strict C compilation and Clang analysis pass for the
identity executable; the Nix wrapper's unused linker argument is suppressed.
These are native consumer changes, not a rerun of the full runtime GPU suite.
ASan/UBSan additionally pass the eight-frame Radeon N=512 counted identity case,
with exact image/geometry agreement. LeakSanitizer is disabled; these are not CPU
leak-detection results. [Sanitizer logs and analyzer output](results/graphics-draw-identity-validation-2026-09-30/asan-stderr)
are retained together. Local sanitizer artifacts are in
`/tmp/ogpu-identity-asan-cez6z8g4`.

## Decision and boundary

Select [explicit indirect ranges and GPU counts](indirect-draw-ranges.md), with
identity local to each public operation. This decision is justified by preserving
native strategies and information, not a local timing threshold. Implementation
must preserve identity when optimizing; arbitrary fusion/splitting is not assumed.

The probe observes the last visible record, not every vertex invocation/ordinal.
Zero-count holes, multiple range operations, padded strides and nonzero starting
offsets remain required public acceptance cases. No changing pipeline topology,
general device-generated commands, Metal mapping or performance benefit is proven.
Next implement the coordinated public range/count migration and its lifetime/
capability/rejection gates, then matched stable-slot timing and installed handoff.
