# Cross-backend API shape: lessons from the DX12 branch

Design assessment, 2026-10-30. Based on DX12 implementation `34930b0` and the
[review handoff](dx12-review-handoff-2026-10-30.md). **Proposed contract revision,
not implemented API or backend acceptance.** The accompanying
[latest NGPU comparison](ngpu-comparison-2026-10-30.md) tests this assessment
against an independently implemented Vulkan/Metal interface.

Decision amendment, 2026-10-30: apply the [minimal composable core rule](design.md#minimal-composable-core).
The mismatches below remain design inputs; the earlier prescription to preserve
each native representation through parallel profiles is superseded. The
[C1–C3 roadmap](roadmap.md#minimal-core-revision) first derives alternatives from
one simple mechanism. Multiple roots, public descriptor bytes, ranged barriers,
exclusive sharing and explicit image layouts are not individually justified merely
by their native availability. Retaining or removing them requires a concrete
composition analysis; no implementation changes are claimed here.

## Conclusion

We do not need the Windows fixes before identifying API-shape problems. Some
review findings are implementation errors, but others show that faithfully
implementing the current public semantics can require work that a native API
does not require. That is already a structural performance counterexample worth
addressing; a timing threshold is not needed to justify a better API alternative.

The foundation remains viable: explicit storage, prepared code, independent
argument updates, commands, dependencies and caller-controlled lifetimes. The
problem is that the public header sometimes treats a Vulkan-specific realization
of those concepts as universal. Comments mentioning optional profiles are not
enough when the actual operation contract still requires that realization.

The correction is a focused cross-backend contract revision, not a wholesale
restart and not merely cleanup of the DX12 implementation.

## Bugs versus abstraction failures

Missing barriers, success returned for failed presentation, dangling surface
ownership and treating device removal as successful completion are implementation
bugs. They do not invalidate explicit dependencies, surfaces or completion objects.

Recipe materialization during heap binding, restoring an emulated shared argument
bank and representing DXGI through the Vulkan presentation protocol are different:
they are evidence that the implementation is trying to satisfy the wrong common
contract. Fixing their immediate correctness bugs would not remove the forced
work or make their semantics native.

Responsibility lies in both layers. The candidate handoff allowed API changes,
but the concrete shared header remained Vulkan-shaped. The next Windows handoff
needs a precise native-profile contract, not another request to preserve those
semantics more carefully. Implementor workarounds are not proof that every
alternative native mapping is impossible; identify the exact mismatch first.

## Contract questions and candidate shape changes

The right-hand column identifies semantics to resolve, not a predetermined number
of public profiles or entry points.

| Current assumption | More general fundamental contract |
| --- | --- |
| Roots occupy bytes in one shared inline bank, preserved across executable changes | Prepared argument interfaces with explicit binding locations, native update operations and compatibility/invalidation guarantees |
| Descriptors are caller-writable encoded bytes in ordinary memory | Explicit descriptor storage, slot/range updates and binding; byte-addressable storage and opaque native heaps are different supported representations |
| Queue domains and stage-scoped submission points follow Vulkan semantics | Explicit queues and dependency operations with discoverable ordering, ownership and completion semantics; no invented physical queue topology |
| A batch inherits one native call's acceptance/failure behavior | Complete local preflight plus specified execution progress/uncertain-acceptance rules where native operations are not atomic |
| Presentation uses Vulkan acquisition, replacement and retirement semantics | Optional native lifecycle profiles with distinct availability, render completion, presentation retirement and image-reuse guarantees |
| Copy footprints can be accepted without native constraints | Queryable layout/alignment requirements and direct commands for supported layouts; preparation/conversion is explicit and outside ordinary copy |

### Arguments and executable compatibility

Retain the simple sequence: prepare an executable/interface, update selected
arguments, issue work. Distinguish semantic argument roles from physical binding
locations. An address root, native buffer binding and inline constants need not
occupy the same byte namespace on every target.

- Let executable preparation select and validate an argument profile and exact
  interface, including native budgets and shader metadata. A device-wide flags
  field alone cannot describe compatibility between two prepared interfaces.
- Make preservation across executable changes explicit. Compatible native
  interfaces may reuse state; incompatible changes may invalidate it. Do not
  restore state automatically to simulate a stronger universal guarantee.
- Start with one logical argument interface. Derive multiple data groups and
  update policies in user code; distinguish root count from inline versus
  referenced observation semantics and from update/launch coupling. Preserve
  efficient reuse through the smallest mechanism that expresses it.
- Reassess the Vulkan shared-byte representation as well as native slot models;
  neither is mandatory. Do not manufacture compatibility with a host shadow and
  root-argument replay.
- Generated tooling can provide portable source-level helpers and exact native
  layouts. It must not conceal a new runtime cost or silently change the
  execution-time versus recording-time observation of data.

### Descriptor storage and mutation

The important common properties are explicit storage, caller-owned slot reuse,
independent updates and cheap binding. Arbitrary writable descriptor bytes are
one useful representation, not a necessary definition of a descriptor heap.

Select one storage model with explicit descriptor creation/update/copy semantics.
Opaque storage is a concrete candidate. Storage is prepared outside recording;
binding must not scan
or rebuild the heap. Specify exactly when changes become visible, which slot
mutations can overlap pending work and what native ownership obligations apply.
Check GPU-selected descriptor movement and caller-controlled placement against
that model. Public bytes are one possible mechanism, not a separate requirement
if the same capability and efficient execution are otherwise expressible.

### Queues, dependencies and submission outcomes

Preserve needed scheduling freedom; do not infer that both global and ranged
barriers are required merely because both exist natively. Queue-wide waits are a
distinct
operation/guarantee from stage-scoped waits, not an invisible conservative lowering.
Likewise, native resource sharing/residency rules are not always queue-family
ownership transfers.

Separate validation atomicity from native execution atomicity. Validate all local
records before side effects, but account for a native wait/execute/signal sequence
failing partway through. The contract must say what is known accepted, what remains
uncertain and when the device enters conservative recovery. Do not promise retry
merely because a later native call reports an allocation failure.

One native call is not itself the goal: an efficient native strategy may require
several calls. Compare with that strategy, not a Vulkan call-count template.

### Presentation and transfer layouts

Keep windows/input outside OGPU and presentation optional. Use real native window
types/profiles, not HWND encoded as a Wayland surface. Prefer explicit native
resize/replacement and drawable/swapchain lifetimes over simulated equivalence.
A rendered frame, retired presentation payload and reusable image are separate
facts. The chosen profile must define them separately.

Copy layout queries should expose footprint size, row/placement alignment and
capability exceptions. Unsupported tight layouts must not silently become hundreds
of row commands or an internal staging allocation. An application/tooling helper
may prepare an appropriate layout explicitly.

## Addresses: hardware, API and toolchain are different boundaries

Distinguish at least these capabilities:

1. A GPU address accepted by a host command or binding operation.
2. An address directly dereferenceable in shader code.
3. Shader pointers loaded from other GPU data, including arbitrary nested
   structures and GPU-produced references.

The branch's pinned Slang/DXC probes reject the nested-pointer strategy. That is
evidence about the tested artifact path, not proof that the physical GPU cannot
perform such accesses or that every future compiler/driver interface must reject
them. Do not promote this toolchain result into a permanent hardware theorem.

Keep physical shader pointers first-class. A descriptor-index-plus-offset profile
may enable other applications, but is a different representation with potentially
different costs, not an invisible replacement. Declare its artifact and data ABI.
Applications using the unrestricted strategy may not run unchanged on a constrained
backend; honest rejection is preferable to a misleading portability claim.

The original long-term question remains: if OGPU had direct native implementations,
could it preserve the efficient hardware strategies? Compatibility backends are
valuable tests of the contract, not authorities entitled to delete capabilities
that their own interfaces cannot express.

## Universality without lowest-common-denominator design

We cannot promise identical application/shader representations, every native
capability and no additional mandatory work simultaneously across arbitrary
underlying APIs. Prioritize native expressibility and avoidance of forced work,
sharing representation wherever semantics genuinely match.

This does not mean three unrelated APIs with backend-prefixed names. Define a
small number of mechanism/guarantee profiles that multiple backends can implement.
Treat a profile as a precise contract, not permission for every backend to invent
different meanings for the same function. Feature availability, selected artifact
ABI and exact operation limits must agree.

"One way to use each hardware feature" is a useful preference against redundant
representations of the same strategy. It must not collapse genuinely distinct
native strategies. The minimal design has few independent concepts and little
mandatory policy, not necessarily the fewest functions or one representation.

## Next design work and evidence

Follow C1 before accepting the architecture fixes in the DX12 handoff:

1. Select the simplest argument/interface contract after analyzing user-level
   composition, observation semantics, native access and invalidation rules.
2. Select one descriptor storage/update model after checking CPU/GPU mutation,
   placement and independent slots. Show creation, update, bind and destruction
   on each backend; justify any additional primitive separately.
3. Specify queue ordering and partial-submission failure dispositions.
4. Specify typed surfaces, lifecycle guarantees and transfer-layout requirements;
   derive convenient frame loops outside core.
5. Turn the contracts into shared semantic tests and backend lowering tests.

Correctness-only Windows fixes can proceed independently. Source inspection
already establishes the need for these contract decisions; actual implementation
and native tests are still needed to prove each selected mapping. Particularly
important are disjoint pending descriptor updates, cross-interface argument reuse,
cross-queue ordering, partial failures and presentation retirement. No universal
performance guarantee follows from a finite test suite.

The [Metal proposal](m6-native-mapping.md) already identified the argument,
residency and presentation boundaries independently. The latest NGPU comparison
adds implemented evidence and identifies which of its simplifications are useful
for us and which would narrow OGPU's fundamental scope.
