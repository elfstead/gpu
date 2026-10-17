# Consolidated foundation contract review

Completed 2026-10-13 after [M4](textured-scene-plan.md#clean-m4-offscreen-acceptance)
and [M5](m5-results.md), before starting M6. This replaces the old ABI20 concern
table, not its historical evidence. The [current design](design.md) describes the
implemented model; [whole-API design](whole-api-design.md) remains the broader target.

Review rule: compare a strong native strategy, the best expressible public
strategy and today's implementation. Preserve native choices under equal
correctness, memory and latency requirements. A structural counterexample is
enough to change the API; no benchmark threshold excuses a forced disadvantage.
No finite suite proves universal native-performance equivalence.

## Dispositions

| Concern | Native strategy and implemented foundation mapping | Disposition / remaining boundary |
|---|---|---|
| GPU-driven work/replay | Native direct/indirect/count draw and indirect dispatch; caller-selected arguments/count spans and explicit list replay | **Preserved for tested cases.** Native draw identity and mutable inputs work without CPU readback. Changing arbitrary command topology on the GPU remains unsupported; replay is not device-generated commands. |
| Argument/state reuse | Separate native push-data byte updates and address roots; bind-once and partial updates are independent of dispatch/draw | **Adopted better API alternative.** No mandatory per-operation root resupply or pointee copy. Same shared native namespace, not independent stage banks. Bind-before-update validation and prepared artifact footprints remain explicit profile rules. |
| Descriptor independence | Batched native descriptor-byte encoding, caller-owned storage/ranges and explicit heap binding | **Preserved at native granularity.** Unused nonreserved slots can change while another slot is pending; four host encoders work on independent ranges. Reserved bytes remain unavailable until referring command buffers reset/die. No whole-heap freeze, clone, registry or implicit upload. |
| Allocation/placement/aliasing | Exact memory-type requests; freely partitioned linear backing; placed optimal images in opaque backing; explicit discard/barrier alias reuse | **Preserved for these backing kinds.** Mixed buffer/image backing, imported/raw-address command endpoints and cross-alias content preservation remain unsupported/unvalidated. This is a real surface gap, not an approved permanent abstraction limit. |
| Host concurrency | Independent native pools/arenas, object-local mutation rules and immutable device data | **Global serialization removed.** Four-worker creation/query/encoding/recording tests pass. Sharing a mutable arena/queue still needs explicit host synchronization. No retained device-wide command/resource graph. |
| GPU concurrency | Exact queue domain/count requests, stage-scoped wait/signal points, ranged barriers and explicit ownership transfers | **Scheduling choices expressible; overlap benefit unproven.** Radeon cross-family transfer/readback passes; llvmpipe lacks the second family. Native queue overlap and cross-queue alias stress remain evidence gaps, not automatic passes. |
| Recording/submission storage | Reserved native command-buffer slots and caller scratch; single/batched submits preserve native records | **No OGPU hot-path heap allocation or mandatory resource walk.** Driver allocation remains possible. Per-call checks/translation and native command costs are implementation work; no zero-overhead claim follows. |
| Prepared execution/state | Explicit pipeline preparation, independent native caches, fail-if-compilation-required, supported static/dynamic raster state | **No command-time compilation or hidden variant cache.** More dynamic state, pipeline binaries and native artifact forms remain open. Pipeline-cache bytes are not portable binaries. |
| Numerical/compiler freedom | Trusted SPIR-V plus explicit optional feature/local/shared/subgroup requirements; pinned generated Slang interfaces | **Preserved for declared profiles.** M5 uses baseline storage16/FP32 by explicit choice. Matrix tuple discovery exists, but no local cooperative-matrix execution. Dynamic shared-memory launch is unsupported, not emulated with hidden JIT. |
| Synchronization/loss | Exact scopes and explicit same-queue split objects, caller retirement, immediate destruction | **No hidden hazard analysis or waits.** Timeout is not retirement; narrow signal is not whole-list completion; unknown submission failure poisons the device. Synthetic failures are not actual device-loss/recovery evidence. |
| Portability | Common concepts with explicit backend/artifact-specific profiles | **M6 mapping proposed, foundation backend not implemented.** See the [native mapping](m6-native-mapping.md). Existing Metal compute is legacy, not foundation. Do not emulate Vulkan replay/inline semantics or hide Metal residency work to manufacture parity. |

## Code and execution basis

Reviewed the public contracts in [ogpu_next.h](../include/ogpu_next.h) against:

- [commands.rs](../crates/ogpu/src/foundation/commands.rs): independent arena pools,
  explicit inline/root updates, native direct/count commands, reusable scratch,
  whole-batch local preflight and one native submission call. The root path copies
  eight address bytes, not the pointee; range validation uses span metadata, not
  a GPU-address lookup table.
- [descriptors.rs](../crates/ogpu/src/foundation/descriptors.rs): independent
  destination ranges, caller scratch, local rejection before native encoding;
  native failure can partially write destinations. There is no rollback staging
  or retained slot map.
- [memory.rs](../crates/ogpu/src/foundation/memory.rs),
  [images.rs](../crates/ogpu/src/foundation/images.rs) and
  [native.rs](../crates/ogpu/src/foundation/native.rs): explicit native ownership,
  requested placement, opaque image backing and immediate destruction. Device
  loss is atomic; no device-wide recording mutex or wait-on-drop.
- [executables.rs](../crates/ogpu/src/foundation/executables.rs),
  [graphics_pipeline.rs](../crates/ogpu/src/foundation/graphics_pipeline.rs) and
  [executable_cache.rs](../crates/ogpu/src/foundation/executable_cache.rs):
  cold preparation and caller-chosen cache synchronization; hot commands do not
  manufacture pipelines. [splits.rs](../crates/ogpu/src/foundation/splits.rs) and
  [secondary.rs](../crates/ogpu/src/foundation/secondary.rs) preserve explicit
  event/child-list ownership rather than retaining a dependency graph.
- [foundation.c](../examples/foundation.c) and its focused fixtures:
  pending disjoint descriptor writes, changed-root/indirect replay, differently
  shaped A/B/A image aliases at one nonzero placement, scoped/secondary state,
  cross-family transfer and guards. GGML/libplacebo scheduling policies live in
  their consumer adapters, not the runtime.

Fresh verification at clean runtime/source `5c2f899`: 46 ordinary foundation-build
tests and 16 GPU tests per driver pass. The full public-C foundation fixture passes
on Radeon; llvmpipe passes with its explicitly declared combined depth/stencil
copy/readback exclusion (`OGPU_FOUNDATION_STENCIL_RASTER_ONLY=1`). That is not full
stencil-copy acceptance. Local logs: `/tmp/ogpu-consolidation-{host,radv,lvp}.log`
and `/tmp/ogpu-consolidation-c-{radv,lvp}.log`.
Final strict foundation Clippy, four ML receipt/measurement tests, seven installer
tests, the package-profile test and all three GGML SPIR-V contract checks also pass.
Changed-document local links and whitespace checks pass; hosted CI is not claimed.
M4 supplies exact-artifact native comparison and range/replay integration evidence;
M5 supplies independent numerical and isolated-SDK evidence. Existing historical
timings are not silently recertified for foundation.

## Remaining fundamental surface gaps

These are **unsupported or unvalidated**, not approved permanent restrictions.
No consumer milestone can prove them irrelevant to a general low-level API.

1. **Backing and interoperability:** mixed linear/image backing, virtual/sparse
   mappings and explicit import/export/native-handle ownership. Span metadata is
   useful for the current backend but does not justify forcing foreign/raw-address
   resources into an allocation registry. Add concrete native-compatible profiles
   before claiming these strategies are available.
2. **Execution families:** device-generated topology, mesh/ray/video, tile-local
   raster strategies, presentation and richer launch/artifact interfaces. The
   draft lists these modules, but even extension-table negotiation is not yet
   implemented. A placeholder declaration is not support.
3. **Numerical and compiler evidence:** broader float-mode requirements, unsupported
   matrix tuples/hardware, additional generated descriptor/layout conventions and
   native Metal artifacts. Shader validity and requirements are trusted producer
   obligations; introspection does not supply safety by itself.
4. **Portability and safety tooling:** no general safe Rust host wrapper, untrusted
   shader sandbox, complete debug/capture interface or portable recovery protocol.
   Real loss, noncoherent hardware and broader physical vendors remain untested.
   Optional safety helpers must not impose retention, copying or synchronization
   on the fundamental C path.

Do not force these into M6's first mixed-workload mapping merely to finish a feature
checklist. Equally, do not stabilize a scope that silently claims them. The roadmap
keeps their explicit entry/verification requirements; the absence of additional
hardware limits evidence, not the design's extensibility.

## Outcome and M6 boundary

Retain the explicit foundation for the implemented profiles. The old one-queue,
device-global-serialization, mandatory-root-resupply and whole-heap-freeze claims
are obsolete. No new fundamental disadvantage was identified in the audited
implemented paths that calls for another core rewrite before M6. This is a
bounded review judgment, not a proof that every future native strategy fits.

The known gaps above remain visible and changeable. Current safety is appropriate
to a trusted driver-level interface, not Rust-safe merely because its runtime is
written in Rust. Stronger safety belongs in an opt-in, explicitly restricted layer.

Consolidation is complete with these dispositions. Next is the
[M6 entry handoff](m6-entry.md): map foundation semantics to Metal 4 and obtain
native acceptance without hiding incompatible mechanisms. No Metal implementation
or native validation was performed in this consolidation. M7 packaging/adoption
and stabilization remain separate decisions.
