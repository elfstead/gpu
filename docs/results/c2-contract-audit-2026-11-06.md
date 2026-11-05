# C2 completed: installed foundation revision 7

2026-11-06, Vulkan/Linux on `minimal-core`, following public cutover `e4e2693`.
The selected C1 foundation is implemented and its migration/deletion audit is
complete. **C2 is complete for Vulkan/Linux; C3 local acceptance follows.** This
is neither API stabilization nor Windows/Metal acceptance or completion of M8–M11.

## Contract and deletion review

| Family | Installed mechanism / implementation | Removed representation; policy outside core |
|---|---|---|
| Arguments and executable interfaces | Prepared immutable fields; one `set_arguments` range update; native domain/binding information (`arguments.rs`, `commands.rs`, compiler generators) | Per-shader interface pointer, root-slot metadata, separate root/inline setters. User owns grouping, persistent records, pointees and upload timing. No argument shadow or spill. |
| Descriptors | Explicit heap/backing requirements and native-unit locations, write/copy rights and immutable heap info (`descriptor_storage.rs`, `descriptors.rs`) | Raw output addresses and separate heap-bind records. Native host-address translation is an implementation detail, not another public storage model. No allocator/cache or hidden binding-time copy. |
| Command composition/generated work | Caller arenas, explicit heap inheritance, borrowed argument layouts and cold mapping compatibility (`secondary.rs`, `generated.rs`) | Old heap bindings and separate generated inline vocabulary. No mandatory replay expansion, pipeline registry or automatic topology construction. |
| Dependencies | Per-entry scopes, shared caller scratch, exact range/ownership capabilities (`dependencies.rs`, `splits.rs`) | Shared-stage/nested-scratch records and implicit global entries. Empty is empty; explicit execution-only entries survive. User owns ordering. |
| Submission | One counted call, timeline/binary arrays, per-batch acceptance (`submissions.rs`) | Separate single/batch/binary paths and linked binary extensions. Native Vulkan batch boundaries survive; no hidden submission or completion counter. |
| Presentation | Borrowed frames, availability/capabilities, creation/resize state and batch/item effects (`presentation_lifecycle.rs`, `swapchain.rs`) | Index-only acquire/release and boolean/status-only present. No frame scheduler, acquisition registry, host drain, synthetic DXGI signals or universal replacement assumption. |

The review found no remaining C2 production compatibility adapter or temporary
private-engine allowance. Cold prepared metadata and shared native translation
helpers are retained implementation work, not parallel public contracts. The
separate opt-in ABI20/legacy Metal path remains intentionally isolated until its
native replacement is accepted; this audit does not claim it has been removed.

Failure/ownership rules stay explicit: local preflight precedes native effects;
submission acceptance is separate from completion; presentation error is not
proof of rejection; replacement can retire an old chain on failure. Unknown
native effects do not grant safe retry/reuse. Trusted pointers, nonoverlap,
asynchronous lifetimes and pending-use synchronization remain caller obligations.

## Audit improvements and corrections

`tools/check-foundation-surface.py` replaces the old Git-baseline operation check.
The reviewed inventory is `tests/foundation-surface.json`, independent of C1 draft
headers or a historical checkout. The audit compiles all **151 C signatures**
against the actual Rust exports or presentation table, compares **123 installed
records and 76 constants (1,063 values)**, rejects retired identifiers, and can
check the exact foundation symbol set in a built library. Existing native Rust
correspondence tests separately compile-check direct Rust operation coverage and
reject forwarding through C. The new audit is wired into CI; this is configuration,
not a claim that hosted CI ran.

The fuller inventory exposed missing native Rust `DrawArguments` and
`DrawIndexedArguments` wire records. They now match the existing C records,
including the indexed wire order, which differs from direct `DrawDesc`. These
are plain `repr(C)` values with no runtime path, allocation or conversion. The C
ABI/revision is unchanged. The ABI fixture now covers every installed public record,
including the nested depth/stencil clear value, and supports explicit C/Rust field
name correspondence. Combined native/legacy/foundation ABI comparison rises from
1,572 to **2,029 values**. This is layout coverage, not proof of every execution path.

Stale revision-1 support/header/installer-test labels and obsolete “presentation
next” wording are corrected. The SDK now includes the four long-form selected
argument, descriptor, dependency and presentation contracts. C1 draft files and
its old migration inventory remain historical design evidence; draft-to-draft
checks are no longer the runtime acceptance gate.

## Verification and scope

- Installed audit passes, including built foundation export equality; its bounded
  C declaration parser has pointer-qualification, fixed-array and rejection checks.
- All 77 CPU library tests and two native Rust correspondence tests pass. Strict
  no-default-feature Clippy, formatting, two audit tests and eight installer tests pass.
- Pinned Slang 2026.14.1 foundation compute/graphics generation and shader checks
  pass; checked-in generated artifacts require no change. C11/C++17 compilation
  and both layout comparisons pass.
- The public-cutover receipt supplies affected Vulkan, bounded Wayland and isolated
  SDK execution evidence. No GPU lowering changed in this audit. C3 will refresh the
  full available-backend handoff on this committed revision, including external
  GGML/libplacebo consumers; no fresh performance claim is made.

Windows still needs native migration and acceptance of argument binding/storage,
descriptor capabilities, dependencies and DXGI resize/presentation effects, plus
closure of its review findings. Metal stays deferred until a Mac is available.
Those limits do not block Linux C3 or subsequent M8 work.

## C3 review follow-up: output initialization

The acceptance review found output-only C adapters constructing Rust references
to storage that callers may leave uninitialized. Rust requires an initialized
value before forming such references; see the
[MaybeUninit reference-initialization rule](https://doc.rust-lang.org/std/mem/union.MaybeUninit.html#method.assume_init_mut).
Submission outcomes, acquire frames, chain info/resize/pacing results and
submission/split scratch requirements now use raw output writes. Resize forms a
reference only after writing the complete initial outcome. The initialized direct
Rust submission slice and raw C output array share one implementation, with no
temporary array, allocation or preliminary fill. Public signatures and revision
remain unchanged; output preservation on failure is unchanged.

Tests supply `MaybeUninit` storage for local rejection and actual accepted
three-batch submission, and for the presentation/scratch output paths. CPU tests
rise to 78; all 28 GPU library tests pass with Vulkan synchronization validation,
and strict Clippy passes. These are contract regressions plus a source-level Rust
validity correction, not a claim of Miri or exhaustive undefined-behavior checking.
The memory/image requirement records remain initialized in/out records because
their input capacity and array pointer are read by the operation.
