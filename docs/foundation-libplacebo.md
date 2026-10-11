# libplacebo on the foundation API

2026-10-11 migration checkpoint. The pinned upstream processing/shader generation
is unchanged. The default Linux execution adapter now includes only
[`ogpu_next.h`](../include/ogpu_next.h); it does not call the installed receipt,
buffer, heap, kernel or batch APIs. This is the existing bounded libplacebo
consumer, not a general upstream backend or a completed presentation API.

## Ownership and scheduling

- Images have explicitly selected local dedicated backing and independent views.
  Upload/readback staging is persistently mapped, separately owned per frame slot,
  with explicit flush/invalidate. Transfers preserve native texel codes.
- Every pass has two image/sampler descriptor banks and vertex allocations.
  Descriptor bytes are encoded directly into host-visible heap storage. The
  shader's native descriptor stride is derived from size/alignment queries.
  Reservations are outside the payload and cache-atom isolated; reserved bytes
  are never written or flushed by the adapter.
- One explicitly selected combined graphics/compute queue, one timeline, one
  serial arena and two frame arenas replace per-operation receipt ownership.
  Arenas are reserved at device creation. A frame still permits eight operations;
  an extra recording slot permits a rejected recording before the adapter fails.
- The serial path submits/waits per operation. Explicit frame mode submits each
  operation without a host wait; aggregated mode records one list and submits at
  frame end. Collection polls or waits for the last ALL-scoped timeline value,
  then resets the arena before releasing operands or delivering readbacks.
- Shader preparation uses consumer-local post-specialization workgroup/shared
  metadata. Original SPIR-V and constant bits go to native preparation unchanged.
  The pass owns its current executable; pending banks retain the exact variant
  they recorded. Replacement does not destroy another frame's executable.

All of these are adapter policies. Foundation does not infer access through
shader addresses, choose allocations, retain resources, add barriers, synthesize
completion objects or wait during destruction. This caller uses conservative
single-queue dependencies, ordinary GENERAL image layouts (unified layouts are
not required), explicit first-use/discard transitions and copy-to-host visibility.
Those choices are not imposed on other foundation users.

Failed local recordings/submissions are discarded; previously submitted work is
drained before cleanup. Transient poll failure marks output invalid but still
waits before releasing resources. A terminal submission failure or failed blocking
completion wait aborts this test adapter rather than attempting teardown with
unproven GPU completion. This is an explicit fail-stop application policy, not a
runtime wait-on-drop contract. Callbacks release caller lifetimes even on ordinary
failed-frame cleanup, but do not assert valid pixels. They must not reenter.

The historical `receipt_reuses` statistic now counts reuse of a collected bank,
including aggregated frames. No foundation receipt objects exist. Other workload,
resource, submission and callback counters retain their meanings.

## Reproduce and evidence

Use the [integration instructions](../integrations/libplacebo/README.md) and pinned
libplacebo revision `3330a515d62139259c26239014f286e233bd3a5c`. SPIRV-Tools development
headers/libraries are now needed for consumer-local metadata. With one selected
ICD and validation configured:

```sh
bash integrations/libplacebo/check-metadata.sh
bash integrations/libplacebo/run-consumer.sh target/libplacebo-source
bash integrations/libplacebo/run-hdr.sh target/libplacebo-source
bash integrations/libplacebo/run-perf.sh target/libplacebo-source verify
```

Radeon RX 5700 XT / RADV 26.2.4 and llvmpipe 26.2.1 pass:

- Nine SDR A/B/A cases and 36 frames each in serial and two-slot streaming;
  intermediate and final output match the native reference byte-for-byte.
- Nine HDR cases, including unchanged 56/208-byte parameter blocks, RGBA16F
  intermediates and HDR-to-sRGB output; zero pixel differences.
- Existing rejection, capacity, staging/bank/slot reuse, pending-poll,
  submit-failure, queued-specialization and partial-frame destruction checks.
  Additional command-end rejection (both frame modes), unknown-completion
  fail-stop, and exact interior RGBA16 UNORM transfer codes are covered.

Radeon also passes native/operations/aggregated correctness at 64x33, 960x540 and
1920x1080 input in resident and transfer modes (largest output 3841x2161), with
zero differences. llvmpipe covers both modes at 64x33; this is not a claim about
its larger workloads. Validation logs contain no validation errors. The 89 ordinary
Rust tests, strict workspace Clippy and 944-value C/Rust layout check pass.

These are correctness/migration receipts, **not new performance results**. Prior
ABI timing reports do not measure this adapter. No second physical GPU or native
Metal foundation execution was tested.

## Remaining migration boundary

The temporary ABI20 adapter and receipt-timing/compiler/placement diagnostic suite
are [retired with historical replay instructions](libplacebo-diagnostic-retirement.md).
Consumer, stream, HDR and verification/perf executables now support only foundation;
the native Vulkan reference remains independent. A legacy API selection or an old
backend test binary is rejected, not silently accepted as foundation coverage.
Builds share an output directory; do not run/build competing variants concurrently.

The preview now exports an independent `foundation-libplacebo` source handoff.
It includes only this foundation adapter, shared processing harnesses and native
reference—not the installed control or old diagnostic wrappers. A separate pinned
upstream clone, external shaderc/SPIRV-Tools and the installed public header/library
are sufficient; no OGPU checkout, Rust or Slang is needed. The relocated handoff
checks build/runtime identity, missing-loader and stale-source rejection, then
executes the same default lifetime/SDR/HDR/small-scale acceptance. See the
[preview instructions](foundation-quickstart.md#independent-libplacebo-adapter).
Relocated builds and complete acceptance pass on Radeon and llvmpipe, including
space-containing SDK/application/upstream paths. A separate no-GPU run passes
build/dependency, missing-loader and stale-source checks without an ICD selected.
Host-only receipt tests reject missing hash entries and changed SDK/source/binary/
upstream-library bytes; CI runs these without shader-compiler dependencies.
This is local handoff evidence, not independent outside adoption.

Continue default SDK and remaining application-workflow cutover. The libplacebo
consumer no longer needs the installed API; other consumers still do. Old runtime
policies should be removed at their last dependency, not maintained as permanent
backends. Historical timings remain unchanged and do not measure foundation.
