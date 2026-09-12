# GGML follow-up: ownership, fixtures and allocation reuse

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

## Scheduler work

In progress. The next checkpoint will record placement, safe in-place execution,
allocator storage reuse and the API alternatives exposed by this integration.
