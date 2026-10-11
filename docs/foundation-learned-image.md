# Learned-image application on the explicit foundation

Local migration checkpoint, 2026-10-11. Source-only `ogpu_next_*` on Vulkan;
installed ABI 20 remains unchanged. This closes the first substantial mixed
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

- The new caller covers small acceptance and resident/end-to-end reuse, not yet
  timing modes, timestamp instrumentation or the video-scale campaign.
  Incidental timings from the installed control are not performance acceptance.
- The installed caller is retained as an explicit migration control and still
  serves those existing benchmark/SDK workflows. It is not a runtime fallback or
  intended permanent second implementation. Remove its migration-only duplication
  when those workflows and the SDK move to the new contract.
- GGML/libplacebo adapters, wider compiler capability metadata, SDK packaging and
  remaining native feature profiles are still work. Native Metal execution of
  the new foundation is unverified; this checkpoint does not depend on a Mac.

Next migrate the application reuse/timing handoff and source-consumer packaging,
then complete the remaining consumer/SDK cutover and retire the old Vulkan policy
layer. Do not call the whole API complete merely because this application passes.
