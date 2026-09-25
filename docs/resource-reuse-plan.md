# M3: sustained resource reuse and consumer diagnostics

Selected 2026-09-26 after [M2 acceptance](language-direction.md). This is the next
bounded milestone, not completed evidence. Use the existing learned-image stream;
no new GPU, Mac validation window, model, shader algorithm or numerical policy.

## First implementation checkpoint

`c4009c0` adds example-local [ranges and slot-state helpers](../examples/resource_reuse/README.md).
The optimized `-DNDEBUG -Wall -Wextra -Werror` CPU suite passes 23,808 mixed-alignment
reservations and 1,000 generation cycles, including capacity/overflow, stale and
premature reuse, known-unsubmitted abort and quarantined failure/drain transitions.
AddressSanitizer/UBSan and Clang static analysis pass. LeakSanitizer initially fails
because it cannot operate under this environment's tracing; the sanitizer rerun
uses `ASAN_OPTIONS=detect_leaks=0`, with no leak-check claim (the helpers allocate
no memory). Analyzer warnings are the Nix wrapper's unused linker flags.

This implements step 1's bookkeeping, not proof of real completion or GPU failure
handling. Terminal quarantine is never recycled even after drain. Generation wrap
rejects; tickets are slot-local and backing/list/completion ownership stays outside
the helper. Step 2's learned-image integration is next; M3 remains unaccepted.

## Reuse established results

The frontier already measured one/two/three slots and 1,000-frame execution;
the replay, HOST-view and split-dependency work subsequently exposed better
contracts. Do not repeat those designs as open questions. The missing integration
is a sustained mixed workload using the current contract with caller-owned ranges,
bounded reuse, failure handling and useful failure reports. Historical controls
remain controls, not automatic acceptance of a newly integrated policy.

## Commit-sized sequence

1. **Checked consumer ranges and reuse state.** Add an example-local C helper for
   bounded aligned ranges with guards and overflow rejection. Pair it with explicit
   idle/recorded/pending/ready/failed slot transitions and generation checks. The
   helper does not own GPU memory, inspect arbitrary pointers, infer hazards, wait,
   grow storage or promise safety after callers bypass it. CPU tests cover exact
   capacity, mixed alignment/cache atoms, overflow, stale generations, premature
   reuse, rejected submission and failure/drain ownership. No runtime API change.
2. **Integrate the learned-image stream.** Compare dedicated buffers against one
   DEVICE arena and separate HOST upload/readback arenas across one/two/three slots.
   Keep the same logical ranges, guards and per-slot weights initially; sharing
   immutable weights would change policy and belongs in a separate comparison.
   Images and draw arguments stay dedicated. Use actual queried HOST views, not
   whole-buffer copy helpers while other ranges are pending. Round independent
   slot ranges to the returned cache granularity, obey flush/invalidate rules and
   retain CPU-access handles. Both dedicated/arena paths use the same mapped-host
   policy so allocation comparisons do not conflate removal of CPU copies.
3. **Sustained correctness, drain and accounting.** Run 1,000 A/B frames for two
   and three slots against serial output, with wraparound and exact alpha/RGB <=1
   gates. Radeon covers existing 720p and odd video extents; both drivers cover the
   small odd case for 1,000 frames. Check every final output in correctness runs,
   input/weight integrity and all intermediate guards after drain, and M1 full
   small numerical regression separately. Inject known-rejected submit, temporary
   observation failure and safely drained terminal failure; reject premature
   range reuse, never recycle on mere timeout/error. Keep synthetic loss distinct
   from actual hardware loss. All accepted work must drain on every exit path.
4. **Submission/allocator decision and report.** Compare owned reset/re-record
   and serial compiled lists for this fixed-command workload, against matched
   direct Vulkan policies where a new cost/opportunity appears. Reuse the accepted
   host-sensitive replay evidence; GPU-heavy parity alone cannot close it. Measure
   application allocation counts/requested/allocated/peak-live bytes, guard/cache
   padding, setup, steady-state host work, wall throughput and latency/tails. Keep
   validation/tracing outside timing; rotate process order and retain raw samples.
   Separately account for command/CPU storage where observable; do not call it zero.
   Decide whether the helper suffices or evidence exposes a better public allocator
   or submission API. Add a reproducible failure report with revision, backend,
   enabled requirements, executable/source identity, frame/slot/generation, operation,
   resource ranges and retirement state. Begin with consumer labels, not a debugger.
5. **Independent handoff.** Relocate an installed SDK and run the standalone helper/
   reuse example without checkout headers or private Rust APIs. Document caller
   obligations and unsupported backends. Commit the result, then prepare M7's
   experimental-release checkpoint; actual publication is a separate action.

## Bounds and acceptance rules

The range helper manages offsets within existing buffers, not native allocation
binding or image aliasing. Query the actual view before laying out HOST ranges.
If chosen backing capacity cannot fit its granularity, fail explicitly or rebuild
setup before recording; do not infer another allocation's coherence or silently
fall back to per-frame allocation. Record setup retries and peak bytes separately.
Expected application allocations are `1 + 8*slots` for the historical dedicated
layout and `4 + slots` for three arenas plus draw buffer and dedicated images,
provided setup needs no temporary backing. These are hypotheses to verify with
the trace, not assumed results or equal-byte claims.

Stable addresses allow one compiled sequence per slot. A completion makes that
execution's mutable range available only after its dependent CPU consumption;
it does not free backing memory retained by a compiled list or license concurrent
resubmission to the same range. Destroy lists before arena teardown. GPU barriers
remain explicit even after a host wait. Public HOST views do not enforce ownership
or track races, so "premature reuse rejects" describes the tested consumer helper,
not arbitrary pointer writes rejected by the runtime.

No global allocator, automatic scheduling, cancellation, cross-queue API, image
pooling, heap mutation relaxation or Metal emulation is selected by this brief.
P1/P5 and wider P6 remain explicitly open; sustained buffer reuse does not settle
host concurrency, descriptor streaming or arbitrary memory placement. Compiler
subgroup discovery/profile work is assigned to M5's entry requirements in the
[language decision](language-direction.md), not hidden inside this allocator work.
