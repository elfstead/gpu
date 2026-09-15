# Working status and next milestone

Updated 2026-09-15. This page owns current status and selected work. The
[design](design.md) describes the model; the [ledger](experiments.md) records
evidence. The [historical plan](plan-history.md) preserves earlier milestones.

## Current phase

Initial feasibility and two bounded consumer integrations are complete. We have
an **experimental runtime/API candidate**, not a stable API, a portable standard,
a GPU source language or a general graphics/ML platform. Breaking changes remain
allowed when evidence exposes a better API alternative; compatibility is not a veto.

The runtime is Rust over the modern Vulkan baseline, with a language-neutral
C boundary at **ABI 10**. Linux x86-64 only. Use matching header/library/shaders
from one source revision. No release/tag or cross-version stability is implied.

| Area | Current evidence | Important boundary |
|---|---|---|
| Core model | Explicit HOST/DEVICE buffers, addresses, prepared executables, X/Y/Z dispatch, batches, dependencies, terminal completion retirement | One queue, externally serialized calls, trusted shaders and caller-owned pointer lifetimes |
| Graphics/images | Compute and raster share batches; independent heaps, preservation, uploads/readbacks and indirect draws | Narrow offscreen state and formats; no presentation or general rendering backend |
| GGML | MNIST direct/scheduled inference, FP32 and FP16 weights with FP32 arithmetic, both memory placements | Bounded operators/layouts; no FP16 arithmetic or accelerated matrix profile |
| libplacebo | Upstream EWA compute and nearest raster, exact reference comparisons, bounded two-frame reuse and frame batching | Bounded adapter, not a general libplacebo backend |
| Performance | Controlled native comparison; image allocation correction `1f41d7e` closes the large resident bottleneck | Near-4K grouped 563.54 vs native 593.17 fps on this GPU; smaller workloads retain larger gaps, not isolated API overhead |
| Validation | RX 5700 XT / RADV and llvmpipe, 30 ordinary tests, 20 GPU tests per driver, 745 ABI layout checks | One physical GPU; synthetic capability/memory tests are not other-hardware evidence |

The [performance diagnosis and correction](libplacebo-diagnosis.md) are complete.
Keep runtime image-memory preference and consumer-side batching. No allocator
framework, runtime scheduler, new public placement flag or further performance
target was selected. The failed shaderc optimized-heap diagnostic remains a known
toolchain limitation; ordinary shader compilation is unchanged.

## Completed work: reproducible two-consumer checkpoint

1. Reconcile current design, support boundaries and build instructions, keeping
   chronology in the ledger/historical plan rather than the active status page.
2. Build from a fresh source checkout with a separate, initially empty target
   directory. Run ordinary tests, ABI/mock checks, GPU tests and both consumers
   through documented commands. Record dependencies, pins, reused input caches,
   exact coverage and any manual setup; do not call this a clean-machine test.
3. Finish with a version-identified checkpoint and explicit remaining limits,
   without freezing the API or automatically widening its surface.

No additional GPU is available. Cross-vendor and physical UMA validation are
**deferred coverage limits**, not blockers for this work. No runner provisioning,
hardware acquisition, remote CI deployment or external coordination is selected.
Use the existing Radeon and software driver where useful; do not manufacture
portability claims from software execution or mocks.

Complete: [checkpoint and reproduction](checkpoint.md), tested at `798e186` with
later documentation-only reconciliation. The fresh checkout rebuilt the runtime
and both consumers without old build artifacts. All 30 ordinary/745 ABI/mock
checks pass; all 20 GPU tests and libplacebo acceptance pass on both available
drivers. GGML's full four-variant matrix passes on Radeon, with a representative
DEVICE/F16 six-case cross-check on llvmpipe. Installed tools, source/data caches
and driver caches were reused explicitly; this is not clean-machine provisioning.
No runtime/build-script changes were needed to complete that checkpoint.

## Decisions blocking the candidate

None for the current bounded checkpoint. D0–D5 are resolved or explicitly deferred;
their [original decision history](plan-history.md#decisions-blocking-the-candidate)
is not a new work queue.

- D0: GGML and libplacebo are the selected consumers, each with a bounded brief.
- D1: Explicit baseline/enabled capabilities and caller-checked executable
  requirements; general reflection and optional accelerated profiles remain deferred.
- D2: Explicit buffer placement/copies; image backing is runtime policy. No
  implicit migration or centralized allocator.
- D3: Shared image/graphics model with documented narrow support. Broader formats,
  views, raster state and concurrent heap mutation need a selected use case.
- D4: Consumer-managed scheduling/reuse; terminal observation retires submission
  resources. No hidden collector or runtime-owned graph scheduler.
- D5: Source revision plus matching ABI/header/library/shaders identify this
  experimental checkpoint. Breaking C layouts/signatures increment the ABI.

## Selected next use case: HDR-to-SDR processing

The [bounded HDR brief](libplacebo-hdr.md) selects a native-first libplacebo capture,
then the smallest coherent extension for floating-point resize and tone mapping
to SDR. It declares input representation, tone/gamut policy, numerical gates and
scope exclusions before results. This uses the existing Radeon and llvmpipe;
additional physical hardware is not a prerequisite.

Native capture is now complete on both drivers, but its original exact
intermediate-alpha gate fails on upstream rounding. It also exposes a renderable
RGBA16F target requirement and a 208-byte raster parameter block that the current
adapter cannot accept. No runtime/API change yet. The brief records the evidence
and proposed format/root-packing/alpha-gate decisions; resolve these before the
OGPU implementation, rather than reporting the native capture as HDR acceptance.

## After the checkpoint

Beyond the selected HDR brief, no further implementation is automatically selected. Choose a concrete user
workflow before widening the API: broader graphics, accelerated ML and a source
language are separate directions. Remaining small-workload overhead, descriptor
compiler optimization and broader tooling are potential implementation work,
not grounds to restart feasibility indefinitely.

Any new experiment needs a named design decision, alternatives, a discriminating
check and a stopping condition. Pure tuning needs its own scope. Stabilization
requires stronger portability, safety/contract and compatibility evidence; see
[the gates](design.md#evidence-required-before-stabilization). Another physical GPU
or backend may eventually supply that evidence, but is not a prerequisite for
finishing the selected local checkpoint.
