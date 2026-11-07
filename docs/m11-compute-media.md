# M11 advanced compute, artifacts and media

Implementation starts 2026-11-07 after [M10 consolidation](m10-consolidation.md).
The hardware batch is optional. C and direct Rust advance together; no feature
becomes a device baseline merely because it is useful to this milestone.

## Work order and concrete exits

| Work package | Native mapping and selected interface | Exit evidence |
| --- | --- | --- |
| 64-bit component formats | Append the twelve pinned core formats to the ordinary catalog; preserve exact image/vertex/storage capability queries | Complete registry disposition, byte-accurate transfer/view tests where native format use exists; shader use needs its own declared numeric requirements |
| GPU conditionals and primitive counters | Native predicate-address begin/end, explicit secondary inheritance; extend existing query pools with primitive-generation kind and independent feature variants | Predicate zero/nonzero/inversion, secondary propagation, no CPU decision/readback; primitive counts independent of transform feedback, discard/nonzero-stream variants separately queried |
| Compute shader profiles | Audit remaining narrow storage, subgroup/control-flow, explicit workgroup layout, derivatives and matrix profiles against the pinned Vulkan/Slang contract | Supported/enabled/required distinction, exact shader declarations and output checks; specialization remains explicit and cold |
| Launch and matrix execution | Use existing specialization, subgroup choice, address arguments and dispatch first; add records only for native cluster/cooperative/device-enqueue semantics that cannot compose efficiently | A bounded hardware matrix consumer with exact tuple/layout/accumulation permissions; no scalar substitution or automatic precision selector |
| Pipeline artifacts | Explicit native binary identity/data/import/capture and executable inputs; audit ray libraries and capture/replay alongside it | Cold compile/capture/recreate equivalence, mismatched identity rejection, ownership/failure paths; no hidden disk cache, recompilation fallback or invented cross-driver binary compatibility |
| Plane-aware images | NV12/P010-class two-plane and corresponding three-plane/packed/padded catalog entries; per-plane extent/aspect/copy/view/placement and native conversion contract | Plane/pitch/alignment/alias validation, guarded copies and conversion/reference output where supported; no hidden repacking or color conversion |
| Initial codec sessions | H.264/AVC 8-bit 4:2:0 decode and encode as independent profiles; native capabilities, explicit session backing/parameters/reference slots, bitstream ranges and status | Deterministic decoded frame/reference checks, encode bitstream decode-back with declared error bounds, explicit reference lifetime/queue dependencies; no container demux, stream parser, player, automatic rate control or implicit DPB ownership |
| Consolidation | Whole-surface structural-cost review and clean C/direct-Rust SDK | Every selected deliverable has implementation status plus native acceptance status; unavailable hardware is a recorded gap, not a successful test |

The first codec selection is deliberately explicit: H.265/AV1, additional bit
depths/chroma formats and vendor media engines remain subsequent capability
profiles. Plane format support does not imply codec support. Codec-specific syntax
must stay representable without a runtime parser; any choice of standardized
parameter records and external header dependency is recorded before implementation.
A universal video-session object must not hide different native reference,
allocation or parameter-update rules.

## Boundary decisions

Native conditional execution cannot generally be replaced by a CPU predicate
readback without a synchronization/cost change. Primitive-generation queries are
also distinct from transform-feedback counters: forcing capture merely to obtain
a count adds unrelated work/state. Both warrant direct native mechanisms.
[Conditional rendering](https://docs.vulkan.org/refpages/latest/refpages/source/VK_EXT_conditional_rendering.html),
[primitive-generation queries](https://docs.vulkan.org/refpages/latest/refpages/source/VK_EXT_primitives_generated_query.html).

Pipeline binaries let an application own individual compiled artifacts rather
than only an opaque aggregate cache. This is a distinct native persistence/creation
choice; retain the existing cache mechanism only where it expresses a different
native facility. Binary import must preserve exact compatibility and ownership,
not advertise portable machine code.
[Native pipeline binaries](https://docs.vulkan.org/refpages/latest/refpages/source/VK_KHR_pipeline_binary.html).

The initial media profile maps to native queue/session/parameter/reference
operations, not host tensor/image processing. It must compose with M10 interop and
explicit queues. Same-device and external image use do not imply an automatic
ownership transfer or plane layout conversion.
[H.264 decode mapping](https://docs.vulkan.org/refpages/latest/refpages/source/VK_KHR_video_decode_h264.html).

## Assumptions and proof obligations

The current Vulkan backend can expose only capabilities its driver advertises and
its selected device enables. A format ID or compiler-accepted shader does not prove
native execution. Local RX 5700 XT lacks advertised hardware matrix and native ray
profiles; implement/query unsupported paths honestly without inventing acceleration.
Metal/Windows acceptance remains deferred. No second GPU is assumed for progress.

Richer launch facilities require a native mapping audit before choosing records.
Per-launch dynamic workgroup memory, cooperative grids, workgroup clusters and
GPU-created work are not interchangeable. Compilation specialization is an adequate
expression only where the native operation itself fixes that property at pipeline
creation. It must not be presented as equivalent to a native per-dispatch change.
Backend limitations remain explicit capabilities, not restrictions imposed on all
future implementations.

Performance review asks whether a direct native implementation of the contract can
preserve the native workload and control. Reject compulsory extra GPU operations,
allocation, staging, scheduling, tracking or host synchronization. Use targeted
experiments for uncertain mappings and correctness; do not benchmark every obvious
translation step before implementing the coherent surface.

## Working status

M10's primitives and clean SDK are complete within the recorded local scope. The twelve numeric format IDs, metadata and local transfer/clear consumer are
implemented; three image profiles pass and nine are natively excluded. Typed
shader/vertex execution remains separate. GPU conditionals and primitive-generation
queries are implemented through C and direct Rust. Local primary compute/draw
predication and primitive queries without transform feedback pass; inherited
conditional execution is unavailable on this device. The selected shader profiles are now implemented (32 independent native enables;
four bounded artifact consumers pass). The [launch/matrix audit](m11-launch-mapping.md) is recorded and native dispatch
origins now pass. Extended matrix profiles and exact fixed/flexible discovery
are implemented. The bounded accelerated ML consumer is implemented with compiled
artifacts and explicit local exclusion. [Pipeline binaries](m11-pipeline-artifacts.md)
now pass local compute round-trip and graphics recreation.
[Ray libraries/capture-replay](m11-ray-artifacts.md) are implemented, with clean
explicit emulation diagnostics and native hardware acceptance open. Native
execution graphs are next, then plane-aware images and media. No
hardware-matrix acceptance is claimed.
The remaining rows are scheduled work, not implemented claims or vague examples.

## GPU execution controls (2026-11-07)

Feature IDs 143–147 independently expose conditional execution, inherited
conditional execution, primitive-generation queries, query support with rasterizer
discard, and nonzero query/rasterization streams. Two recording operations borrow
a four-byte predicate range; an optional inheritance extension preserves the
existing secondary-record ABI. One new query kind uses the existing pool,
reset/begin/end/resolve and explicit stream argument. The primitive feature does
not require enabling transform feedback. Its extension dependency and stream limit
come from native transform-feedback *extension* metadata, not an active capture.

The public contract names exactly which operations are predicated. Draws, compute
dispatches and generated-command execution are covered; copies/fills, barriers,
queries, rendering load/store, acceleration builds and direct ray dispatch remain
unconditional. Predicate values stay stable within a scope, with explicit
CONDITIONAL stage/read dependencies. Local scope checks prevent unmatched/nested
begin/end; discard and rasterization-stream restrictions remain documented unsafe
caller obligations. No extra work, CPU decision, retention or implicit dependency
is introduced. Recording has constant-time local checks and one native call;
query slots are reserved with the arena, not allocated while recording.

The independent [Rust consumer](../examples/foundation_rust/src/bin/controls.rs)
enables `[RASTER, CONDITIONAL_RENDERING, PRIMITIVES_GENERATED_QUERY,
PRIMITIVES_GENERATED_DISCARD]`, deliberately excluding TRANSFORM_FEEDBACK. GPU
fills produce predicates; zero/nonzero and inversion produce four checked compute
outputs, four primitive counts and four unconditional fill markers, with surrounding
4096-byte guards and five local rejection cases. Vulkan synchronization validation
is clean. Conditional inheritance is implemented and its native chain has host
coverage, but RADV reports that feature unavailable: the consumer's secondary
execution branch is **not accepted**. Nonzero stream emission, predicated mesh/
generated work and attachment clears likewise need separate execution evidence.

An explicit `OGPU_CONDITIONAL_RESET_DIAGNOSTIC=1` run reuses a native slot after
abandoning an open predicate and resetting the pool. It produces one
`VUID-vkCmdBeginConditionalRendering2EXT-None-13063` diagnostic despite OGPU resetting
its local scope. Matching VVL 1.4.357 source at
`e4786f7ce8f1319215eff0d938f4be4651cbb85d` leaves `conditional_rendering_active` out of
`CommandBuffer::ResetCBState()` (cmd_buffer_state.cpp:305); it is set/cleared only by
conditional begin/end (2519/2526). This supports a stale validation-state diagnosis.
The normal negative-case fixture uses separate reserved slots; it does **not**
establish abandoned-scope same-slot validation acceptance. No workaround or extra
native end command is inserted into OGPU. The raw diagnostic is kept separately
from the clean execution receipt.

[Receipt](results/m11-execution-controls-2026-11-07.json).

## Shader capability profiles (2026-11-07)

IDs 148–179 add independent uniform/push/I/O narrow storage, scalar and standard
uniform layouts, variable pointers, dynamic subgroup broadcast IDs, subgroup
rotation, uniform control flow, maximal reconvergence, quad control, explicit
workgroup layouts and narrow workgroup storage, compute derivative groups,
expect/assume and float-controls-2, terminate/demote, and remaining core image and
clip/cull shader permissions. Core 16-bit storage-buffer access remains baseline;
sample-rate interpolation already maps to the existing sample-shading feature.
There is no second alias for either mechanism.

The existing required-feature list and immutable physical queries carry these
profiles. `SHADER_PROFILE_LIMITS.derivative_stages` reports compute and, where
advertised, mesh/task derivative applicability; local value is compute only.
Subgroup operation discovery now includes native rotate/clustered-rotate bits.
Layout decorations, derivative groups, convergence modes, numerical permissions
and correctness of expect/assume assertions remain the artifact author's explicit
responsibility. OGPU does not rewrite SPIR-V, add barriers, infer alignment,
choose precision, or allocate shared memory during a launch.

Native extension dependencies do not silently enable shader features. Quad-control
enables its required maximal-reconvergence extension, but the independent feature
bits still match the caller's list. Broader variable pointers explicitly require
the storage-buffer variant, matching native valid usage. Request every capability
the artifact declares; workgroup scalar/narrow permission does not invent the base
explicit-layout capability or integer arithmetic capability.

All 32 new features pass individual native device creation with validation on
local RADV. That is **enablement acceptance**, not shader execution acceptance.
The [standalone consumer](../examples/foundation_rust/src/bin/shader_profiles.rs)
executes four Slang 2026.14.1 artifacts:

- subgroup and four-lane-cluster rotation, with lane/width-derived exact references;
- linear derivative groups, checking derivatives `(1, 2)` for every invocation;
- 2D quad derivative groups, checking `(1, 8)` for every invocation;
- byte and halfword push storage, preserving native offsets 8/10 and separately
  enabling the integer arithmetic used by the artifact.

Each executes 64 invocations with two input values, checks all payload outputs and
surrounding 4096-byte allocation guards, and rejects the same declared requirements
on a device that did not enable them. SPIR-V capabilities/modes match the requested
profiles; synchronization validation is clean. Uniform/I/O narrow storage, scalar/
standard layouts, variable pointers, uniform/reconvergent/quad control, explicit
workgroup block accesses, expect/assume, float-controls-2, terminate/demote, image
operations and clip/cull outputs still need their own execution witnesses.

C/direct-Rust records and constants match. The source SDK includes both M11
consumers and all embedded shader inputs; its relocated checker exposes
`--execution-controls --shader-profiles`. Full clean SDK consolidation remains a
milestone exit, separate from this implementation/consumer batch.
[Receipt](results/m11-shader-profiles-2026-11-07.json).

## Intermediate source SDK checkpoint

Clean source `74d3771c1ca68d34f794a20a0211b3f90eee879a` installed at
`/tmp/ogpu-m11-profiles-sdk` and rebuilt independently at
`/tmp/ogpu-rust-external-v1bdp4fx`. All nine consumer binaries built; depfile and
linkage checks kept every input in the relocated source bundle. Headless,
native interoperability, primary execution controls and four shader-profile
artifacts passed with validation, as did host and missing-loader checks.
This checkpoint predates the dispatch-origin addition and is not M11 completion.
[Receipt](results/m11-profiles-sdk-2026-11-07.json).

## Extended matrix profiles (2026-11-07)

Feature IDs 180–192 expose workgroup scope, flexible dimensions, reductions,
accumulator conversions, per-element operations, tensor addressing, block loads,
BF16 type/dot/matrix permissions, FP8 type/matrix permissions, and opt-in native
matrix robustness. The native feature chain sets exactly the requested fields;
an extension dependency does not silently enable a shader capability. Scalar
arithmetic/storage and the original cooperative-matrix feature remain independent.
These are shader permissions, not a host tensor object or a numerical selector.

Fixed tuples now preserve subgroup/workgroup scope and BF16/E4M3/E5M2 component
IDs; supported shader stages use the existing public stage mapping. New physical
queries expose flexible tuples with joint dimensional granularities, component
and saturation constraints, and exact workgroup invocation counts. A separate
limit record exposes maximum dimension, workgroup invocation limit and reserved
shared bytes. The caller/compiler checks native artifact constraints, including
subgroup multiples and shared storage; OGPU does not round dimensions, pad inputs,
choose precision or allocate scratch. Existing fixed tuple layout is unchanged.
The public component IDs are independent of native enum values.

Host checks cover exact feature fields, extension dependencies, preservation of
low-precision components and joint flexible constraints, and bounded enumeration
with initialized native headers/retries. Local RADV exposes none of IDs 180–192:
all thirteen requests correctly return UNSUPPORTED; fixed/flexible tuples are
empty and the profile limits are zero. This establishes discovery/rejection,
**not native enablement or matrix execution acceptance**. Future hardware must
validate those paths and low-precision numerical behavior separately.

Native references: [matrix profiles](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceCooperativeMatrix2FeaturesNV.html),
[flexible tuples](https://docs.vulkan.org/refpages/latest/refpages/source/VkCooperativeMatrixFlexibleDimensionsPropertiesNV.html),
[BF16](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceShaderBfloat16FeaturesKHR.html),
[FP8](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceShaderFloat8FeaturesEXT.html).

[Matrix profile receipt](results/m11-matrix-profiles-2026-11-07.json).

## Bounded accelerated ML consumer (2026-11-07)

The independent [Rust MLP consumer](../examples/foundation_rust/src/bin/matrix_ml.rs)
implements `Y = ReLU(X W1 + B1) W2 + B2`: batch/input/hidden/output dimensions 16,
row-major FP16 input and weights, FP32 accumulation and bias, and an explicitly
permitted FP16 hidden representation. It selects only a complete advertised
16x16x16, subgroup-scope, compute, nonsaturating FP16/FP32 tuple. It separately
requests native cooperative matrices, FLOAT16, Vulkan memory model and exact/full
subgroup control. Neither a smaller tile nor a flexible/workgroup/low-precision
alternative is substituted. Those profiles need their own artifact witnesses.

The caller owns all storage, both address roots, compiled pipelines, three recorded
dispatches and dependencies. The second matrix receives the GPU-produced hidden
array; there is no intervening host readback. ReLU and the half conversion are a
visible consumer kernel, not implicit backend staging. The same list executes
A/B/A input phases, with changed weights too. Both FP32 outputs and the FP16 hidden
array are checked against a double-precision host reference; input/root bytes and
all unused allocation bytes must remain unchanged. Values are small exact binary
fractions whose intermediates fit the chosen formats exactly. This tests dataflow,
layout and changed-input reuse, not arbitrary-data numerical error or throughput.

Both real Slang artifacts compile and pass Vulkan SPIR-V validation. Host tests
verify reference representability and reject tuple/type/scope substitutions.
Local RADV lacks feature 46 and the exact tuple; public discovery returns an
explicit unavailable result before allocation or executable creation. **No native
matrix numerical acceptance is claimed.** The source SDK includes the binary and
artifacts; `test-rust-install.py --matrix-ml` requires successful real execution
on a future suitable host. This is a bounded acceleration witness, not replacement
acceptance for the complete M5 transformer or quantized consumer.

[ML consumer receipt](results/m11-matrix-ml-2026-11-07.json).

## Pipeline binaries (2026-11-07)

The [artifact contract and evidence](m11-pipeline-artifacts.md) records individually
owned native binaries, identity/key queries, cache-only lookup, explicit capture
and release, byte export/import, source-free compute/graphics/ray creation, and
optional disabling of the native internal cache. Compute output equivalence and
graphics recreation pass locally. The undersized-capture threading-tracker
diagnostic is retained separately; ray execution and imported raster output remain
unaccepted. Ray libraries/capture-replay and execution graphs still precede media.

## Ray artifact package (2026-11-07)

[Contract, lifetime obligations and evidence](m11-ray-artifacts.md) now cover
explicit libraries, nested/imported-stage linkage, stable handles and opaque
capture/replay. The caller owns the dependency lifetime graph. Normal local RADV
rejects unsupported profiles; explicit emulation passes fresh-device replay and
exact output/guard checks with clean validation. Mixed creation and native ray
hardware remain unaccepted. This completes the implementation package, not M11.
