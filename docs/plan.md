# Working status and first integration milestone

Updated 2026-09-13. This is the authoritative near-term work plan. The
[design](design.md) describes the model; the [ledger](experiments.md) records
evidence. Neither a backlog entry nor a successful benchmark schedules more work.

## Current phase

The [modern Vulkan baseline migration](modern-baseline.md) is implemented and locally
verified (`39b8c16`). [Modern compute and GGML now pass on physical RADV hardware](hardware-validation.md);
physical graphics/heaps and provisioned execution-CI checks remain pending.
There is no Vulkan 1.2 compatibility path. The [retention/retirement comparison](retirement.md)
is complete: adopt completion polling and optional whole-buffer retention, while
the application decides when scratch ranges are reusable. Timeline limits and
failure recovery are verified; no centralized allocator or retirement queue was
selected. The [heap-indexed image experiment](heap-images.md) passed the compiler
and public-API execution gates: compute image access and fragment sampling need no
intermediate image-to-buffer copy. Both checkpoints from the
[ownership/preservation review](image-ownership-review.md) are now implemented:
[cross-submission preservation and LOAD/CLEAR](image-preservation.md), then
[independent image/sampler heaps](descriptor-heaps.md) with exclusive mutation,
copied descriptions and configurable sampling. The old coupled table API is removed.
The current interface is **ABI 3**; rebuild callers with the matching header/library.
Local contract and regression gates pass. The RX 5700 XT passes modern compute and
GGML, but its driver lacks `unifiedImageLayouts`, so physical graphics/heap validation
is still open. Next is selecting a full-profile GPU host and authorizing runner
provisioning, not another unrelated workload. The current host can support a separately
scoped compute-only lane, not the full hardware gate.
Concurrent slot streaming, generalized formats/views and compute-only image deployment
remain deferred scope decisions, not hidden requirements to finish this checkpoint.
The completed integration checkpoints below remain historical regression evidence;
the new hardware result and its limits are recorded separately.

Initial feasibility is complete. We have a working Rust/Vulkan runtime and C ABI,
with tested compute, mixed graphics/compute, cooperative reduction, FP32 matrix
kernels, and optional batch timing. See the ledger for exact coverage and limits.
This supports moving to an **API candidate for integration**, not declaring a
stable API, a portable standard, or a complete graphics/ML system.

The first consumer is [GGML's FP32 MNIST forward graph](consumer-ggml.md).
Its brief resolves candidate decisions by retaining the current bounded API.
The first integration checkpoint is complete: `375f333`, with paired-driver
acceptance recorded in the brief. There is no release date or stability claim.

"Experimental" now means breaking changes are permitted and limitations are
explicit. It does not mean an indefinite sequence of workload demonstrations.
Existing experiments remain regression tests and diagnostic tools.

## First usable scope

Deliver one documented, version-identified runtime/API checkpoint that a selected
consumer can build against and use without host-side Vulkan calls or private
backend access. This is an integration target, not a performance or adoption claim.

The initial scope is Linux x86-64 with the existing Rust/Vulkan backend, address-
based compute, prepared executables, explicit dependencies, and completion-owned
submission resources. Keep graphics in the shared model through an optional,
explicitly limited offscreen profile; keep timing optional. A compute-only device
must not be excluded by either optional profile.

The checkpoint must describe unsupported operations and features rather than
silently weakening their semantics. It must distinguish supported capabilities
from what an execution device actually enables. Existing trusted-shader and
caller-managed pointer-lifetime obligations remain visible.

Not promised by this milestone: a new GPU source language/compiler, presentation,
a general rendering engine, all ML numeric types or matrix instructions, multiple
queues, multiple backends, tuned GEMM performance, or stable cross-version ABI.
These exclusions bound the checkpoint, not the project's eventual ambition.

## Intended model versus stepping stones

"Intended" describes a semantic direction, not frozen names or signatures.
"Provisional" means usable today but requiring an explicit retain/revise decision
for this checkpoint. Deferred work is not a hidden prerequisite.

| Area | Intended | Provisional now | Checkpoint treatment |
|---|---|---|---|
| Host boundary | Language-neutral C ABI, Rust implementation, explicit errors | Current function naming and version/discovery conventions | Preserve the boundary; decide compatibility policy in D5 |
| Memory | Owning allocations separate from non-owning GPU addresses | Dedicated host-visible buffers and CPU copies | Resolve placement/transfers in D2; do not present this as the final memory architecture |
| Executables and arguments | Prepared code with explicit requirements; application-owned data layout | Trusted SPIR-V, `main`, fixed root bytes, caller-known tile/workgroup sizes, 1D dispatch | Resolve the minimum executable contract in D1; fixed forms may remain if sufficient and documented |
| Submission and lifetimes | Explicit dependencies and completion; retained directly referenced resources | One queue, one-shot batches, global barriers, externally serialized host calls, draining destruction | Retain as the starting candidate; verify consumer fit in D4 |
| Graphics/images | Graphics and compute share memory/submission rules; specialized images stay explicit | Fixed RGBA8 raster state, independent checked heaps, nearest/linear clamp/repeat sampling, exclusive edits, preserved contents | D3 follow-up implements preservation and independent ownership; broader formats/views and concurrent mutation stay deferred |
| ML operations | Arithmetic and tensor interpretation live in executable/consumer code | Baseline FP32/integer workloads and incomplete feature negotiation | No host reduction/matmul API; accelerated profiles deferred unless required by the selected scope |
| Timing | Optional diagnostics without gating ordinary execution | Whole-batch bracket, per-completion query pool, no calibrated clocks | Keep the current documented limitations; finer profiling and pool optimization deferred |
| Safety/portability | Explicit obligations and unsupported cases | Trusted shaders, manual pointee lifetimes, one backend and limited hardware coverage | No sandboxing or broad portability claim; further backends remain a later validation gate |

## Delivery sequence and exit conditions

| Step | Status | Deliverable | Done when |
|---|---|---|---|
| F0 — Feasibility | Complete | Working model and recorded workload evidence | Existing ledger establishes the implemented paths; unanswered product questions move to the decisions below |
| C1 — Scope and consumer brief | Complete: [GGML brief](consumer-ggml.md) | One named consumer, revision, use case, and acceptance checklist | D0 is resolved and the brief separates required capabilities from nice-to-haves |
| C2 — API candidate decisions | Complete: retain/defer decisions in brief | D1–D5 decisions, candidate header/contracts, necessary implementation changes | Each decision is resolved or explicitly deferred without contradicting the brief; each included change has tests |
| C3 — Public-interface integration | Complete: [acceptance and friction](consumer-ggml.md) | Reproducible consumer integration and a friction report | The selected workflow works through the public interface and meets its independently stated acceptance checks |
| C4 — Experimental checkpoint | Complete: source `375f333`, ABI 1 | Version-identified source checkpoint, build/use instructions, limitations and compatibility notes | Header, contracts, consumer, and regression results agree; open issues are classified as future work, not unstated requirements |

Consumer inspection during C1 can inform C2; there is no requirement to design in
isolation before looking at integration code. Any later scope change must update
the brief and this plan, rather than quietly accumulating additional gates.

Checkpoint completion does not publish a release or authorize contacting upstream
maintainers. A release/tag or external contribution can be handled explicitly.

The [GGML hardening and scheduler follow-up](ggml-hardening.md) is complete:
teardown was isolated, regression weights pinned, adapter ownership made explicit,
and the same graph now executes through GGML's scheduler/graph allocator with
verified placement, no hidden fallback, and intermediate storage reuse. Direct
and scheduled modes pass the numerical and repeated-lifecycle gates on both drivers.
Source checkpoints: `a53564c` (hardening) and `dae0ba7` (scheduler/reuse).
Review whether the work **exposes a better API alternative** against the project's
goals. Compatibility with the current API is not a veto on a better design, and
an alternative need not be forced by an integration failure to be worth adopting.
The scoped consumer session was adopted. The strongest new public-API candidate
was explicit buffer-range use/retention declarations. Subsequent comparison separates
allocation retention, allocator retirement and access tracking: they offer different
guarantees. The completed [retirement experiment](retirement.md) adopts independent
polling and optional whole-buffer retention, without making resource-use lists
mandatory merely to assist lifetime management.

## Decisions blocking the candidate

Decisions D0–D5 are resolved for this consumer in the [brief](consumer-ggml.md).
The inventory below records the questions those decisions address.

| ID | Question | Evidence already available | Required decision/output |
|---|---|---|---|
| D0 | Who is the first consumer, and what must it accomplish? | Existing examples prove execution, not integration value | Resolved: GGML MNIST forward inference; see the brief |
| D1 | How does a consumer describe executable requirements and choose a compatible variant? | Reduction/matmul share the ABI, but the host knows root layout and tile sizes; reported feature bits are not enabled features | Specify baseline/optional requirements, enabled-capability reporting, entry point/root/workgroup agreement, and failure behavior; say which metadata is declared versus validated |
| D2 | What memory placement and transfer behavior does this consumer need? | Host-visible buffers work; device-local linear data and staging have not been exercised | Choose explicit allocation/copy semantics if needed, or retain host-visible-only with a consumer-supported limitation; document address stability, range/lifetime and completion rules |
| D3 | Which image operations belong in the first offscreen profile? | Buffer-mediated and native heap-indexed graphics → compute → graphics paths work | Consumer checkpoint retained the fixed profile; follow-ups implement direct image access, preservation and independent heaps, leaving general formats/views and concurrent edits deferred |
| D4 | Which submission/ownership model best serves the project? | One-shot state, dependencies, retention and failure cleanup have tests | Walk repeated execution and teardown; evaluate whether the work exposes a better API alternative, including improvements beyond basic compatibility |
| D5 | What identifies a compatible runtime and executable contract? | Fixed-width C layouts are checked, but the ABI remains experimental | Define checkpoint identification, version mismatch/feature-availability behavior, shader-contract identification, and how breaking changes are recorded; no accidental stable-ABI promise |

Do not turn this inventory into a mandate for reflection, a shader package format,
an allocator framework, or a general feature-negotiation language. Decide the
minimum contract needed for the brief, including what is intentionally unsupported.

## Consumer brief and integration acceptance

The selected consumer's brief records:

- Project/revision and the specific application or compiler/runtime workflow.
- A useful result defined independently of this API: actual inputs, expected
  outputs/tolerance, and a reproducible acceptance command.
- Required compute, graphics/image, numeric, memory and completion capabilities;
  target platform and a bounded workload size. Performance is an acceptance gate
  only if the brief names a concrete budget and measurement method.
- The intended integration boundary, allowed adaptation work, and any licensing,
  dependency or build constraints. Reading/evaluating a project does not imply
  authority to contact it or modify an upstream repository.

Prefer a naturally mixed graphics/compute consumer if a suitable one is available;
do not manufacture graphics work solely to satisfy a checklist. A compute-only
consumer can validate the core, but does not validate the optional graphics profile
for real application use. Record that distinction in the checkpoint.

Acceptance requires a separate consumer build using the public header/library and
documented executable contract, with no private Rust modules or host Vulkan escape
hatch. Vulkan-targeted SPIR-V is allowed by the current executable contract; it is
not evidence of backend-independent shaders. Repackaging an existing example in a
new directory does not count as independent integration evidence.

Verify meaningful repeated use and teardown, outputs against the brief's reference,
and unsupported-capability behavior. Record what adaptation was necessary, where
the interface helped or obstructed the consumer, and remaining limitations. A
successful run alone does not establish that this interface is better than its
alternatives. If integration needs an out-of-scope facility, explicitly revise the
scope or choose a better-fitting consumer; do not hide the dependency in Vulkan.

## Rules for further experiments

Every new experiment must name D0–D5 (or an explicitly approved new decision), the
alternatives it distinguishes, the smallest workload/check that can distinguish
them, and a stopping condition. If the outcome would not change a contract choice,
it is implementation/performance work, not a blocking API experiment.

Larger matrix sweeps, more dispatches per submission, resource-pool optimization,
finer timestamps, accelerated arithmetic and broader image benchmarks are parked.
Use existing tests while implementing C2/C3; do not expand benchmarks automatically
because a previous experiment finished. Fixing regressions in promised behavior
remains normal implementation work, not a reason to reopen feasibility.

## Beyond this checkpoint

A real consumer and an experimental API checkpoint are not stabilization. A second
backend, additional hardware/memory configurations, broader graphics/ML profiles,
and comparative performance/integration evidence remain necessary before stronger
claims. See the [stabilization gates](design.md#evidence-required-before-stabilization).
Those are later gates, not requirements to finish this first integration milestone.

Update this page when a step or decision changes status, linking its decision,
implementation, and verification evidence. Keep historical measurements in the
ledger and detailed experiment documents; keep the active next task here.
