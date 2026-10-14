# M6 entry handoff: Metal foundation

Updated 2026-10-15. M5 and the [contract consolidation](core-contract-review.md)
are complete; [P1 presentation](presentation-plan.md) now precedes this migration.
**No M6 implementation has started.** The earlier requested stop at this handoff
is complete. A Mac is not available in this environment;
Linux compilation/mocks cannot supply native acceptance.

## Starting point

- Linux foundation revision 1 is the default SDK, with explicit backing, ranges,
  queues/timelines, caller-owned command storage, descriptor bytes and executable
  requirements. [M4](textured-scene-plan.md#clean-m4-offscreen-acceptance) and
  [M5](m5-results.md) close bounded consumers, not the entire target surface.
- Existing [Metal 4 compute](metal.md) is the legacy ABI20 source path with earlier
  ABI12 native acceptance. It is neither a foundation implementation nor a current
  mixed graphics/compute result. Keep this baseline intact until its replacement
  has native validation; do not turn it into an automatic fallback.
- The shared public concepts need exact Metal mappings, not Vulkan internals
  renamed into a backend trait. Foundation currently lives under Linux-specific
  modules; separate genuinely common record validation from native execution as
  implementation requires, without generalizing unproven backend assumptions.

## First implementation tranche, when resumed

1. **Write the native mapping/profile matrix.** Map placement/mapping, address
   spaces, command allocator/reset/replay, queues/events, barriers, descriptor
   mutation granularity, prepared artifacts and residency. For each, declare
   supported exactly, backend-specific alternative, or unsupported. Inline versus
   device-root arguments and replay are the important early decisions; no hidden
   root copy, command re-recording or wait may disguise a semantic mismatch.
2. **Implement the resource/command core.** Explicit memory, queue/timeline and
   arena lifetimes, loss/rejection behavior and caller-controlled residency where
   native execution needs it. Prove resource lifetime across submissions without
   restoring transitive pointer scanning, mandatory retention or device-wide locks.
   Introduce an explicit extension/profile if the existing surface cannot express
   Metal's efficient strategy; the C API may change when that is the better design.
3. **Connect native executable interfaces.** Generate native MSL/metallib layouts
   from actual artifacts. Do not assume that Vulkan SPIR-V root offsets, descriptor
   byte layouts or argument namespace map unchanged. Keep optional translation a
   tooling choice, not the definition of the common programming model.
4. **Run the mixed learned-image workload.** RGBA8 offscreen rendering, compute-
   to-fragment address reads, indirect fullscreen draw, preservation, copy/readback
   and explicit dependencies. Add the heap-image sampling consumer next. Validate
   changed interfaces, A/B/A, reuse, rejection and teardown on a Mac. Keep optional
   timestamps separately queried; no window/presentation dependency is needed.
5. **Map presentation and run the interactive consumer.** Implement the optional
   P1 boundary with native Metal drawable/layer integration. Window/input handling
   remains external. Specify drawable acquisition, backpressure, image lifetime,
   render completion versus presentation completion, format negotiation and
   resize behavior without pretending Vulkan swapchain semantics are universal.
   Validate multiple frames in flight, resize/minimize/restore and safe teardown
   on a Mac. Offscreen tests remain independent of a display.

Batch the native work into a coherent handoff. Use the same semantic fixtures and
CPU oracles, but native artifacts and explicitly documented backend profiles.
Unsupported strategies stay visible. Do not claim M4's full textured scene or
M5's numerical profiles on Metal until their own native tests pass.

## Completion and subsequent work

M6 closes only with native mixed-workload, generated-interface, lifetime/reuse
and interactive presentation acceptance. Without a Mac, implementation can be
prepared but the result remains unvalidated. No hardware acquisition or external
coordination is implied.

M7 is an ongoing package/support/upgrade track, with independent integration
evidence sought when authorized. M8–M11 schedule the remaining hardware families;
see the [roadmap](roadmap.md#scheduled-hardware-coverage). Pull explicit residency
or interop prerequisites into M6 if the native mapping needs them, without claiming
the whole M10 package. When Mac validation is unavailable, leave M6 acceptance
open and continue useful Linux work. No release publication, stable ABI or
universal performance claim follows from this handoff.
