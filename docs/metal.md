# Native Metal 4 backend

Updated 2026-09-16 on `metal-backend`. ABI **12**; use matching headers, library
and callers. This is an experimental compute backend, not a portability claim.
The Metal 4 migration and subsequent synchronization, feedback and lifetime
corrections are validated natively on Apple M4 with macOS 26 and Xcode 26.5.

## Boundary and current implementation

Common Rust code owns range/copy/root/dispatch checks, one-shot recording rules,
deduplicated retention, terminal resource retirement, shader-description parsing,
C diagnostics and common ABI data types. Vulkan and Metal use the same submission
state: preparation owns resources, accepted work keeps them, terminal observation
releases them while preserving the result. Transient Vulkan polling errors do not
grant release permission. Native encoders, submission, draining and timing remain
backend responsibilities; there is no trait that mirrors either native API.
The platform C entry-point implementations are still separate; this is a first
contract extraction, not a completed unification of the entire runtime.

Metal implements HOST/DEVICE buffers, GPU addresses, prepared compute kernels,
XYZ dispatch, copies, batches, barriers, explicit retention, poll and wait.
Graphics, images/heaps and timestamps remain unsupported. Creation outputs are
cleared on failure. Allocation checks `maxBufferLength` and the nullable native
buffer result before constructing an owning Rust wrapper. Oversize roots are
rejected before shader compilation; workgroups are checked against pipeline limits.

`OgpuDeviceInfo.backend` identifies Vulkan or Metal explicitly. Vulkan API-version
fields and Vulkan-only feature-audit flags are zero on Metal. In particular,
`timeline_semaphore` and `synchronization2` are not portable tests for OGPU completion
and barrier behavior. The common execution contract guarantees that behavior.
Native numeric capabilities remain distinct from storage-access capabilities.

Residency is private to the backend. A device residency set tracks allocations as
they are created and destroyed, so dispatch no longer scans every live allocation.
Metal 4 argument tables bind the root-data GPU address; embedded pointers remain
opaque. `retain_buffer` means ownership retention, not a new resource-access
declaration language.

Aligned copies use native blits. Unaligned offsets/lengths use a small cached MSL
byte-copy kernel, including disjoint self-copies; overlapping self-copies fail.
Validated zero-byte copies encode and retain nothing. This preserves OGPU's byte
contract despite [macOS blit alignment constraints](https://developer.apple.com/documentation/metal/mtlblitcommandencoder/copy%28from%3Asourceoffset%3Ato%3Adestinationoffset%3Asize%3A%29).

## Executable artifacts, not a mandated source language

`OgpuShaderDesc` now uses a byte blob, byte size, format, entry name and local size.
All inputs are consumed during creation; the ABI does not expose SPIRV-Cross types.

| Artifact | Vulkan | Metal | Metadata |
|---|---|---|---|
| SPIR-V | Native | Optional SPIRV-Cross adapter | `main`, zero `local_size`, existing 32-bit specialization |
| MSL source | Unsupported | Native compilation | Entry name, nonzero XYZ `local_size`, pre-specialized |
| metallib | Unsupported | Native library loading | Entry name, nonzero XYZ `local_size`, pre-specialized |

Native Metal root bytes occupy `constant ... [[buffer(0)]]`; embedded pointers are
native GPU addresses. Other argument bindings are not supplied. Native artifacts
currently reject specialization lists; they do not pretend that untyped
SPIR-V `{id,bits}` values specify every native function-constant type.
SPIR-V translation lives in `metal/spirv.rs`, outside native pipeline preparation.
Build with `--no-default-features` to omit that adapter and its C++ dependency.
Native copies and native executable tests do not depend on it.

## Metal 4 execution model

Execution requires Apple Silicon / Apple7-family features and the Metal 4 API
(macOS 26+). Each batch uses a Metal 4 command allocator and command buffer with a
single compute encoder for dispatches and copies. Each barrier uses dispatch/blit
stages and device visibility in three scopes: intra-encoder, queue consumer and
queue producer. This covers earlier commands in the current pass, earlier
submissions, and consumers in later submissions (including a barrier-only batch).
Copies may lower to either stage, so the stage masks are deliberately conservative.
See Apple's [consumer barriers](https://developer.apple.com/documentation/metal/synchronizing-passes-with-consumer-barriers)
and [producer barriers](https://developer.apple.com/documentation/metal/synchronizing-passes-with-producer-barriers).

Per-commit [Metal feedback](https://developer.apple.com/documentation/metal/mtl4commitfeedback)
establishes terminal success or failure. Its callback copies the native diagnostic
into shared Rust state; it never accesses `Rc` resource owners. Poll reads that
state, wait sleeps on a condition variable, and observation retires resources on
the calling thread. Errors report `INTERNAL_ERROR` with the native code/description
and a zero Vulkan result. The synchronous dispatch convenience uses the same
submission/completion path. No timeline-event success inference or busy-wait loop
remains. As before, no timeout or recovery from a driver that never reports
completion is promised.

Submission moves the command buffer and backing allocator out of the batch;
terminal observation releases them even if both public batch and receipt handles
survive. Public buffers and internal root/copy uploads share checked nullable
allocation. Descriptor objects also have scoped ownership on failure paths.
There is no classic-command-buffer compatibility path.

## Acceptance handoff

On Apple Silicon with Xcode's Metal compiler installed:

```sh
cargo test --locked -p ogpu
cargo test --locked -p ogpu metal:: -- --ignored --nocapture --test-threads=1
cargo test --locked -p ogpu --no-default-features
cargo test --locked -p ogpu --no-default-features metal::native_tests -- --ignored --nocapture --test-threads=1
cargo clippy --locked -p ogpu --all-targets -- -D warnings
cargo xtask smoke
cargo xtask compute
cargo xtask batch
cargo xtask reduction
cargo xtask retirement
```

Enable Metal API validation when running the native tests. They fail rather than
silently pass when a GPU/compiler is missing. Tests cover translated specialization,
MSL and compiled-metallib execution without translation, copied root data,
destroyed kernel handles, one-shot submission, repeated receipt observation,
buffer-size/root-size rejection, byte copies through private memory, overlap and
zero-copy rules, live-registry cleanup and cached-failure poll outputs. New tests
cover barriers at the producer end, consumer start and in an intervening empty
batch, all blit/dispatch copy pairings, retained batch handles, injected nullable
allocations, gated pending polls, owned NSError diagnostics and error retirement.

Native acceptance on 2026-09-16 covered pending polls, destruction of pending
receipts, byte-copy validation, argument-table residency and the new cases above.
The failure-retirement test substitutes an error only AFTER actual native feedback
confirms termination; it does not deliberately fault the GPU.
Real device-loss/error delivery and allocation pressure remain native validation
work, distinct from the deterministic injected-null and NSError tests.

Then run the real consumer with the default SPIR-V adapter enabled (do not use
`--no-default-features` for this step). Install CMake, Ninja, SPIRV-Tools and ripgrep;
use the pinned GGML checkout and dataset steps in [the consumer README](../integrations/ggml/README.md):

```sh
bash integrations/ggml/prepare.sh
bash integrations/ggml/run.sh /path/to/pinned/ggml 0 device f16
```

The harness selects `.dylib` on macOS, accepts extra arithmetic capabilities,
uses portable core-limit/checksum handling and disables GGML's own Metal backend.
Expected gates: lifecycle checks, two mixed-matrix cases, and six full-dataset
direct/scheduled inference cases without fallback. Keep the numerical tolerances
unchanged; report any failure for investigation. The Apple M4 DEVICE/F16 run passed
the lifecycle gates, both mixed-matrix cases and all six 10,000-image inference
cases without fallback (9,801 correct, zero changed predictions, maximum FP32
drift 0.00321006775). This is bounded consumer acceptance, not a performance or
images/graphics claim.

Linux verification and exact remaining coverage are recorded in [the plan](plan.md).
Apple-target `cargo check` can catch Rust errors here using `DOCS_RS=1` to skip the
SPIRV-Cross C++ build; it cannot validate Apple SDK linking, MSL compilation or GPU
execution and is not a substitute for the commands above.
