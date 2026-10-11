# Learned-image application on the explicit foundation

Local migration checkpoint, 2026-10-11. Experimental `ogpu_next_*` on Vulkan;
default installed ABI 20 remains unchanged. This closes the first substantial mixed
application migration, **not** the whole foundation revision or SDK cutover.

## What moved

The [public-only caller](../examples/learned_image/foundation.c) executes the same
frozen learned-image workload: hidden activations, denoising, conventional resize/
color processing, fullscreen raster output, and diagnostic poisoning/readback.
It consumes the same six Slang sources and the shared compiler adapter's generated
foundation records. There is no old-runtime wrapper, shader rewrite, runtime
reflection or tensor/operator API.

Allocation, memory-type selection, queue choice, mapping/cache visibility and
retirement are application-owned. The caller requests one graphics+compute queue,
allocates explicit linear memory for inputs/weights/intermediates and separate
dedicated image backing, then creates a borrowed color view. LOCAL memory selection
does not assume discrete VRAM or forbid host-visible local memory.

Setup uploads weights and initializes guard regions once. One command list per
normal/diagnostic mode records explicit upload, compute, compute-to-fragment,
render-to-copy and host-read dependencies. Lists replay across all same-extent
inputs, including A/B/A, without re-recording per image. Timeline completion gates
host reuse; reset/destruction are explicit. The fragment reads compute-written
linear storage directly: no intermediate image conversion, CPU read or host wait.
Diagnostic copies occur after the chain, not between its stages.

The reuse handoff also supports `--validate resident` and `--validate end-to-end`.
Both preload two host fixtures and execute A/B/A. Resident mode uploads each input
once during setup and records two lists with distinct input addresses; normal work
contains neither an input upload nor a pixel readback. A separate snapshot list
copies its result for acceptance only. End-to-end mode reuses one list with mapped
upload/readback storage. Diagnostic lists copy the selected resident input, not a
stale staging buffer. Nothing is re-recorded between A/B/A frames.

The compiler supplies layouts, embedded artifacts, local dimensions, feature
requirements and one common graphics inline bank. Named fields survive reversed
root order and 64-to-32-thread compute changes. Application launch tiling, pointer
bounds, allocation sizes, image format and numerical policy are not generated.

## Acceptance

`cargo xtask foundation-learned-image` runs the existing frozen quality/numerical
fixture checks, generates original/mutated foundation interfaces, compiles the
same foundation C source for both, and freshly builds the installed caller as a
migration control. Its existing file-comparison/oracle logic is reused without
changing thresholds or replacing CPU references with GPU goldens.

Both Radeon/RADV and llvmpipe pass all 38 cases in normal and diagnostic modes
for each of the three callers: **228 checked frames per driver**. Within each
driver, every final and intermediate output byte agrees between foundation,
mutated foundation and installed control. Inputs and weights stay unchanged;
guards and A/B/A distinction/reproduction pass.

The reuse follow-up passes 36 more checked frames per local driver: both modes,
normal/diagnostic A/B/A, original/mutated foundation and installed control at
65x47 to 131x95. All 21 output files match across those six runs, with the same
frozen oracles and guard checks. These checks are included in the normal command.

For both local driver runs, the maximum scalar error is `1.71016513e-7`, maximum
error/allowed-bound ratio is `0.00667210166`, and maximum RGB code difference is
one. Alpha is exact. These are current-run observations, not cross-driver bitwise
or future-driver guarantees. Vulkan and synchronization validation are enabled;
no stencil-copy diagnostic exclusion is needed for this color-only application.

`cargo xtask foundation-learned-image --check` passes CPU, generation and strict C
build checks without GPU execution. All 21 existing application-interface tests
also run against the foundation artifacts. Ordinary Rust tests, Clippy, compute/
graphics generator reproduction and dependency-receipt checks are local regressions.
The frozen model, inputs, shader sources and oracle tolerances are unchanged.

Use the normal foundation Vulkan environment, pinned Slang 2026.14.1, SPIRV-Tools,
Python 3 and C/Rust tools. The runner reuses installed tools and Cargo/driver caches;
it is not a clean-host setup test and downloads nothing. Runs sharing generated
outputs should be serialized.

## What this does not finish

- The new caller covers small acceptance, resident/end-to-end reuse and explicit
  replay timing instrumentation, not yet the video-scale campaign or a new
  matched-native comparison.
  Incidental timings from the installed control are not performance acceptance.
- The installed caller is retained as an explicit migration control and still
  serves those existing benchmark/SDK workflows. It is not a runtime fallback or
  intended permanent second implementation. Remove its migration-only duplication
  when those workflows and the SDK move to the new contract.
- The [GGML adapter](foundation-ggml.md) now has a Linux foundation path. Libplacebo,
  wider compiler capability metadata, default SDK cutover and
  remaining native feature profiles are still work. Native Metal execution of
  the new foundation is unverified; this checkpoint does not depend on a Mac.

An [opt-in preview](foundation-quickstart.md) now exports this caller and its model/
independent scalar reference, alongside compute and graphics examples. It checks
12 resident/end-to-end frames without a checkout, Cargo, Slang or preexisting
fixtures. This does not migrate the whole installed SDK or remove the old runtime.

Next migrate the remaining application workflows,
then complete the remaining consumer/SDK cutover and retire the old Vulkan policy
layer. Do not call the whole API complete merely because this application passes.

## Explicit replay timing handoff

`cargo xtask foundation-learned-timing` builds the three callers and checks the
reuse fixtures with synchronization validation first. It then disables layers for
four small instrumentation runs (two foundation interfaces, two modes), each with
10 warmups and 30 reported alternating-input frames. `--check` runs only host/
generator/build checks. Reports and raw stdout/stderr stay under fresh
`target/learned-image/foundation-timing-*` directories; incomplete reports never
claim completion. Source, header, library and executable hashes identify the run.

Normal lists reset two explicit query slots, write timestamps at ALL scopes,
resolve 64-bit values and availability to a separate 32-byte mapped allocation,
and record COPY-to-HOST visibility. An initial ALL-to-work dependency follows the
first timestamp. The last timestamp precedes the result resolve. Timeline
completion and cache invalidation precede host decoding. Query storage is reused
serially, never overlapped; pools and result memory outlive the lists. Queue valid
bits mask subtraction, and host duration must remain below the counter's wrap
interval. Clocks with no finite >10-second interval produce explicit `null` device
times rather than rejecting an otherwise usable device. This is fixture policy.

Recording is setup work, separately reported from device/pipeline/input setup.
Per-frame host intervals cover input copy/flush, submit, wait, query invalidate/
decode and final-image invalidate, with no per-frame allocation, recording, arena
reset or object destruction. Resident mode has no pixel readback allocation in
measurement runs. Its only GPU-to-host copy is the optional timestamp resolve;
normal end-to-end work also copies pixels. End-to-end completion means pixels are
available in the persistent mapping, **not** copied into another CPU allocation.
File I/O, snapshots and diagnostic poisoning/copies are outside measurement runs.

The receipt accounts for requested linear-memory bytes and logical image bytes,
not native allocation granularity, driver bookkeeping or physical residency.
The host total includes query resolve/completion overhead; the device interval
does not include the resolve itself. These are serialized instrumented latency
samples, not throughput, a native Vulkan comparison, or a replacement for the
older matched-control performance acceptance. That control records/retire batches
per frame and has different host-copy/instrumentation boundaries. Do not compare
their numbers as if the workloads were matched.

Both local drivers pass the instrumented 36-frame reuse validation and all four
40-frame timing smoke runs. Timestamp values are available and positive in every
reported sample. Four parser tests reject count/order, mode/accounting, nonfinite
clock and wrap errors; C helper tests cover 36/64-bit wrap, high-bit masking and
invalid/ambiguous intervals. No numerical or public-API contract changed.
