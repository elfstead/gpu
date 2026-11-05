# M8: GPU-culled generated graphics through the minimal core

2026-11-06, following [local C3 acceptance](c3-linux-acceptance-2026-11-06.md).
A bounded generated-graphics consumer now executes through foundation revision 7.
**No runtime, public API or ABI change is needed.** This is the first graphics
execution checkpoint for the existing generated-command primitives, not M8
completion, a general scene engine or a performance result.

## Composition and coverage

`examples/foundation_generated_graphics.h` and
`examples/shaders/foundation-generated-graphics.slang` form a small consumer in
`cargo xtask foundation`. A compute shader culls a procedural 8-by-4 grid against
a column window and stably compacts visible objects into caller-owned draw records.
It writes the sequence count, object/argument data and optional pipeline indices.
Generated vertex/fragment work draws six vertices per visible tile, with colors
that encode object identity, native sequence identity and executable selection.

Five cases vary the window across zero, one, four, eight and three visible columns.
They cover empty work, compacted identity, changed GPU records/counts, full capacity
and an absent optional count span. Fixed-executable and table-selected variants
have separate capability gates. Each supports explicit preprocessing outside the
render pass and native integrated preprocessing at execution. Selection-capable
drivers also exercise a changed executable-set slot on the final case.

The host records one producer dispatch and one generated execution per case;
there is no count readback, CPU draw expansion or submission between them.
Dependencies, preparation storage, image transitions and completion remain explicit.
The consumer waits only for post-execution checking and safe reuse between cases.
Its serial one-invocation compactor is a deterministic correctness fixture, not a
recommended large-scene algorithm. Users can replace it with parallel compaction
without changing the OGPU contract.

## Local result and capability finding

AMD Radeon RX 5700 XT, RADV/Mesa 26.2.5, Vulkan validation and synchronization
validation enabled:

- **10 submissions, two preprocessing modes, 20,480 independently expected pixels
  pass**, including empty frames and a changed visible set after full capacity.
- Every byte of each 32 KiB host-visible allocation is compared: generated records,
  count, image readback and untouched guards. The native sequence token supplies
  identity rather than a producer-written replacement.
- The full public C fixture passes with no validation errors. Existing shader-output
  warnings elsewhere in that fixture remain warnings, not newly waived errors.
- Slang 2026.14.1 reproduces and SPIR-V validation accepts all three new artifacts;
  the full foundation shader reproduction check passes. C11 builds with warnings
  treated as errors; Rust formatting passes.

This driver supports generated vertex/fragment execution but **does not expose
graphics pipeline selection**. An initial combined capability gate skipped both;
the fixture now gates them independently and runs fixed-executable graphics.
Table selection/set updates compile and have expected-output logic here, but remain
natively unverified for graphics. Existing generated compute selection evidence is
separate. No fallback, feature emulation or extra GPU is assumed.

Reproduce with `cargo xtask foundation-shaders --check` and `cargo xtask foundation`
under the declared Vulkan/validation environment. The new fixture is source-tree
acceptance; it is not yet a separately installed consumer bundle.

## Remaining M8 work

Follow-up: the [indexed/nested-count receipt](m8-generated-indexed-counts-2026-11-06.md) closes the first two execution gaps below, corrects native shader index semantics and moves preparation queries after set updates. The remainder of this section records the initial checkpoint.

Generated indexed draws, nested-count graphics actions and mesh-generated work
still need execution coverage; physical task/mesh and graphics selection remain
unverified locally. Vertex/index-state tokens and the remaining advanced raster
profiles remain implementation work. A larger textured/depth scene and external
consumer packaging can build on this fixture. None of those gaps is closed by the
bounded procedural grid, and no speed or general native-performance parity claim
is made. The result supports keeping culling and compaction outside the core.
