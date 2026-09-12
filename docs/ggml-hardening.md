# GGML follow-up: ownership, fixtures and allocation reuse

Completed checkpoints: `a53564c` (scoped ownership, fixed fixture and teardown
isolation) and `dae0ba7` (scheduler placement, safe aliasing and storage reuse).
The public C ABI remains version 1; the local GGML-facing session API changed.

## Design question and stopping condition

Evaluate whether ordinary GGML scheduling, tensor lifetime reuse and application
ownership expose a better API alternative for the project's low-level graphics/
compute/ML goals. An alternative need not be forced by failure of the current
API. Compare explicit ownership and range/lifetime representation, not merely
whether the old interface can execute this workload.

First harden lifecycle and fix regression inputs. Then run the same pinned MNIST
graph through GGML's scheduler/graph allocator with measured storage reuse,
verified node placement and no hidden CPU fallback. Keep the original numerical
and prediction gates and repeated validated execution/teardown. No new ML operator
family, asynchronous scheduling or performance target is part of this follow-up.

## Teardown investigation

The [minimal reproducer](../integrations/ggml/teardown.cpp) has no GGML dependency,
shaders, buffers or submissions. Compile once against OGPU and once with
`DIRECT_VULKAN` against headers and `dlopen`, with **no OGPU/Rust library**:

```sh
c++ -g -std=c++17 -Iinclude integrations/ggml/teardown.cpp \
    -Ltarget/release -Wl,-rpath,"$PWD/target/release" -logpu -o target/ogpu-teardown
c++ -g -std=c++17 -DDIRECT_VULKAN -Ivendor/Vulkan-Headers/include \
    integrations/ggml/teardown.cpp -ldl -o target/vulkan-teardown
```

Set `OGPU_VULKAN_LIBRARY` to the absolute loader path for the direct caller.
Modes: `explicit` destroys before returning from main; `static` destroys a global
owner constructed before the loader is opened; `atexit` registers cleanup after
device initialization. `static` is an opt-in fault reproducer, not a CI success
test. Run under a debugger or with core dumps disabled.

Observed with loader/layers 1.4.357.0, Mesa 26.2.1/RADV, Linux x86-64:

| Caller | Explicit, validation on | Static, validation off | Static, validation on |
|---|---|---|---|
| OGPU | Pass | Pass | Abort in validation device lookup |
| Direct Vulkan | Pass | Pass | Same abort and validation lookup stack |

The backtrace reaches `vvl::GetDispatchDevice` → `DestroyDevice` while running
`__run_exit_handlers`. Matching validation source at
`e4786f7ce8f1319215eff0d938f4be4651cbb85d` defines static owning `device_data` and
`instance_data` maps and explicitly diagnoses late exit-time access after its
static memory has been destroyed. See
[dispatch_object_manual.cpp](https://github.com/KhronosGroup/Vulkan-ValidationLayers/blob/e4786f7ce8f1319215eff0d938f4be4651cbb85d/layers/chassis/dispatch_object_manual.cpp#L611).
The layer is dynamically initialized after the caller's global owner, so its
static cleanup precedes that owner's destructor. Keeping a `dlopen` handle alive
does not prevent process-exit static destruction. In this build the allocator
aborts while the layer attempts to print its missing-device diagnostic.
The direct caller's `atexit` mode, registered after device initialization, passes
with validation: cleanup order, not simply reaching process exit, is decisive.

This isolates an external validation-layer lifecycle/order interaction, not
corruption from our kernels, GGML allocation or Rust child ownership. It does not
establish behavior for every loader/layer/platform. No upstream changes or reports
were made. The consumer uses scoped cleanup before process static destruction;
we do not work around it by leaking the runtime or silently disabling validation.

## Ownership and fixed inputs

GPU-owning globals are replaced by a scoped `OgpuGgmlSession`. Device, buffer-type
and registration callbacks carry explicit context; backend streams and buffers
hold shared ownership of execution state. Constructor failure is transactional
for device/shader preparation; session destruction detects live children before
unregistering/freeing state. Host calls remain externally serialized.
The lifecycle test covers repeated creation/destruction, duplicate registration,
invalid-device and missing-shader initialization, a buffer surviving its backend
stream, and subprocess death tests for premature session destruction with a live
backend or buffer. The runner checks both the abort status and diagnostic.

The original 98.01%-accuracy model is checked in with provenance and a verified
SHA-256. Local and CI regression runs now use exactly those weights. Only the
test dataset is downloaded for routine acceptance. Optional CPU training writes
`mnist-fc-trained.gguf` separately and never replaces the regression fixture.

## Scheduler and allocation-reuse result

The same upstream graph now runs in two modes: direct execution with independently
allocated intermediates as a control, and GGML scheduling with its graph allocator.
The scheduled workflow checks support before allocation, marks weights as weights,
and lets GGML choose placement. Every executable node must resolve to the primary
OGPU backend, with one split. This is checked before execution; each call must
produce exactly five OGPU dispatches. The driver rejects unsupported graphs rather
than permitting the scheduler's normal CPU fallback.

GGML's allocator detects dead input tensors and assigns the same allocation/range
to both bias additions and ReLU. The adapter permits only **exact source-0 aliasing**
for those elementwise operations. An invocation reads/writes its own element;
there are no `restrict` declarations. Partial overlaps, broadcast-bias overlap
and every matrix input/output overlap remain invalid. Compute READ|WRITE barriers
before each dispatch order RAW, WAR and WAW hazards, including reused storage and
previous submissions. This does not infer dependencies from GPU addresses.

Paired local results, 2026-09-12, RX 5700 XT/RADV and llvmpipe, validation and
synchronization validation enabled:

| Batch size | Direct intermediate bytes | Scheduled intermediate bytes | In-place nodes |
|---|---|---|---|
| 1 | 6,272 | 2,112 | 3 |
| 17 | 103,552 | 34,752 | 3 |
| 64 | 389,120 | 130,560 | 3 |

These are GGML backend buffer capacities for intermediates, including tensor
alignment, not total process memory or Vulkan allocation requirements. Weights,
inputs, metadata, driver allocations and the adapter's host token arrays are
excluded. Token storage still adds approximately one host byte per GPU-buffer byte.

All six mode/size combinations per driver covered all 10,000 images: 9,801 correct,
every top-1 prediction matching CPU, maximum absolute logit difference
0.0000343322754, and every logit meeting the original absolute/relative tolerance.
Padded tail logits were checked too. Exact token/buffer assignments were checked
unchanged after every call; scheduler allocation happens once per model, not per
inference. Models, schedulers, buffers and sessions are destroyed/recreated between
mode/size runs. Tests reject unsupported operations, FP16, strided layouts,
matrix aliasing, partial elementwise overlap and bias overlap before submission.
Sentinel output and dispatch-count checks detect partial execution of rejected
backend graphs. A scheduled unsupported-op check verifies refusal before fallback.

Scoped lifecycle tests passed on both drivers, including intentional live-child
death tests. ShellCheck, Rust formatting/Clippy, 20 ordinary tests, seven GPU tests
and 550 C/Rust ABI checks passed. CI is configured to run fixed-fixture direct and
scheduled acceptance plus lifecycle checks; no remote CI run is claimed.

## API alternatives exposed

This review is about better alignment with the project, not preserving the current
API until something forces a change.

| Alternative | Benefit and tradeoff | Decision here |
|---|---|---|
| Scoped consumer session instead of global register/shutdown state | Makes ownership and teardown visible; contexts identify which state each callback uses; child-lifetime violations are diagnosed | Adopted in the GGML-facing API. The runtime already has owning devices/children; no GPU-owning globals are needed in the adapter. |
| Runtime tensor graph/in-place operators | Could hide graph allocation and alias rules, but makes the low-level boundary own GGML-like tensor semantics, duplicates the consumer's liveness knowledge and does not generalize naturally to graphics | Keep tensor liveness and elementwise alias legality in GGML/consumer code. This is a design choice, not a claim that a graph API is impossible. |
| Explicit buffer-range use/retention declarations attached to a dispatch or batch | Could associate non-owning pointers with retained allocations and access ranges, improving lifetime diagnostics and making narrower dependencies possible across compute/graphics | A promising public-API alternative for a separate bounded prototype. Compare retention-only declarations against range+access declarations; preserve arbitrary pointer-root layouts and make incomplete declarations explicitly a caller obligation. |
| Host mapping/import to eliminate the adapter's token arrays | Could remove this integration's extra host allocation, but couples pointer identity, placement and cache-maintenance/lifetime rules; mappings are not generally device addresses | Do not add mapping merely to satisfy GGML's token convention. Evaluate it with the broader transfer/placement design; token reservation without committed backing is a separate adapter optimization, not a solved portable C++ pointer model. |

The public API remains the existing address/explicit-dependency model for this
checkpoint because it preserves consumer-owned interpretation and transparent
synchronization. The scoped adapter API is a concrete improvement adopted now;
range-use declarations are the strongest new public-API candidate exposed by this
work. Their value would be safer resource/lifetime expression, not additional
MNIST operators. Implementing them is not an unstated gate for this completed step.

Still outside the evidence: arbitrary graph topology/views, graph reset/rebuild
within one scheduler, concurrent host callers or asynchronous callbacks,
device-local staging, additional GPU vendors and graphics consumers. No performance
claim follows from the intermediate-storage reduction.
