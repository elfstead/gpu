# C1 P: explicit presentation lifecycle

Selected 2026-11-03; **Vulkan public cutover implemented 2026-11-05 in foundation
revision 7.** Resolves P in the [minimal-core contract](minimal-core-contract.md).
The [installed header](../include/ogpu_next.h) and direct Rust operations now expose
this lifecycle; see the [cutover receipt](minimal-core-migration.md#c2-presentation-public-cutover).
The [C sketch](drafts/ogpu_presentation.h) and [Rust mirror](drafts/presentation.rs)
remain design/check artifacts. Win32/DXGI names are installed but explicitly
unsupported on Linux. No Windows/Metal acceptance is claimed.

## Decision and scope

Keep independent surfaces, explicit chains and caller-owned synchronization.
Acquisition returns a borrowed frame value describing its actual availability.
Presentation reports native acceptance and lease/synchronization effects. Neither
operation constructs a frame scheduler, allocates synchronization, selects a queue,
retains application resources, inserts a host wait or submits bridge work.

Four facts must stay distinct: permission to access the selected image, completion
of application rendering, retirement of presentation's borrowed payloads, and
permission to use an image again. Display timing is a fifth, optional observation.
An API that calls all of these a completed frame cannot express the native rules.

Use the same lifecycle operations with capability-dependent guarantees. Keep native
surface/configuration tokens honestly namespaced: a Vulkan MAILBOX mode is not a
DXGI swap effect or sync interval. The draft retains the existing Vulkan chain
description and adds a tagged DXGI description accepted by the same `chain_create`.
Native options do not create a second scheduler or a second image/command model.
No Windows or Metal execution is claimed by this selection.

## Surfaces and cold setup

Keep the typed Wayland surface entry point and add a typed Win32 HWND entry point.
Discovery advertises/enables supported platforms explicitly. A surface borrows
its native window/display and owns sufficient native factory/instance identity to
survive discovery destruction. Never retain a pointer into the discovery's adapter
array. Windows remain outside OGPU; serialize their lifecycle under platform rules.
The draft adds WIN32=2 alongside the existing WAYLAND=1 discovery platform bit.

Existing surface capabilities/formats/modes/queues queries remain live cold queries.
Their native tokens belong to the selected presentation profile. Do not interpret
Vulkan mode/scaling records as DXGI guarantees. The integrated C1 surface-query
table must distinguish applicable schemas and reject unsupported queries; Windows
adds native configuration capability queries during its implementation handoff.
Creation validates the exact request, even after a successful earlier query.

`presentation_contract_query(adapter, surface, profile)` reports the availability
kind, replacement/resize/retirement/pacing/binary-wait capabilities and native batch
limit. This is a live cold query, not a device-independent portability promise.
The queue/surface/device must match and PRESENTATION must be enabled explicitly.
The batch limit is nonzero and counts distinct chains in one native call;
UINT32_MAX denotes no smaller interface limit. Combining queried chains still
requires a common supporting queue and compatible native batch restrictions.

| Selected mapping | Vulkan with existing maintenance baseline | D3D12/DXGI flip model |
|---|---|---|
| Availability | Native binary and/or fence signals | Current-buffer selection; availability through the associated native presentation queue |
| Presentation dependency | Explicit native binary waits or independently established completion | Native ordering on the queue supplied at creation; other-queue work explicitly ordered by the caller |
| Retirement fence | Native presentation fence | Not advertised by this mapping; ordinary queue fences are not relabeled |
| Native batching | Existing one-call multi-chain batch | One chain per call; no internal loop that claims batch guarantees |
| Replacement retaining old acquired images | Supported | Unsupported by the selected HWND flip-chain mapping |
| In-place buffer resize | Unsupported; use native replacement | Supported with explicit old-image invalidation |
| Frame pacing handle | Not advertised by this mapping | Optional native waitable handle if selected at creation |

The DXGI description names the queue, exact image count/extent/format/usage, native
swap effect/scaling/alpha/color space/flags and maximum latency. Nonzero dimensions
and count are explicit; zero does not select a window size or default. Native
configuration options remain subject to support checks. Maximum latency is zero
when waitable pacing is disabled; when enabled it requests the native limit.
Tagged records use version 1, exact byte size, zero header flags and null `next`.
No configuration is silently substituted. Vulkan keeps its existing minimum-count,
sharing-domain and compatible-mode-list semantics. [Native buffer/queue model][dx-chains]

## Borrowed frame and availability

`frame` contains chain, image, generation, image index, availability kind, ordered
queue and any caller-supplied acquisition binary/fence. It owns nothing. Generation
starts at one and changes when an in-place resize invalidates borrowed images.
Copies do not grant additional acquisition rights. A stale copy must not be reused
after present/abandon, even if the same image index is acquired later. Generation
detects resize staleness; it is not a per-acquisition ownership tracker.
Acquisition binary/fence fields record the supplied identities without extending
their native lifetimes. Present/abandon do not dereference these historical fields;
they validate the chain/generation/image identity and their own explicit sync inputs.

The runtime may keep fixed chain state/generation/current-selection bookkeeping.
It does not allocate a lease object per frame or maintain a resource-retention
graph. Callers prove outstanding acquisitions, synchronization state, aliases and
recording lifetimes. Native Rust exposes the same unsafe borrowing obligations;
an optional safe frame loop may enforce stronger policies outside the core.

For `SIGNALS`, the caller supplies at least one idle unsignaled binary/fence.
Acquisition forwards the timeout under native forward-progress rules. On
OK/SUBOPTIMAL it publishes a frame and schedules those signals, without waiting
for them. First GPU access waits at the appropriate stage, or the caller proves
acquisition complete through its fence. NOT_READY/TIMEOUT produces no frame;
out-of-date, surface loss and device loss remain distinct. Other outcomes preserve
the output record. Neither returning an index nor signaling the render timeline
substitutes for acquisition. [Native acquisition][vk-acquire]

For `QUEUE_ORDERED`, both signal pointers must be null and `timeout_ns` must be
zero. The call selects the native current buffer; it does not wait for display
availability or manufacture a semaphore/fence signal. The frame identifies the
creation queue on which the native presentation ordering applies. It is not a
host-access guarantee or a transferable acquisition signal. Only one outstanding
selection per chain is allowed in this mapping. Present/abandon ends it. Vulkan
retains its native multiple-acquisition freedom and forward-progress limits.

Cross-queue processing still uses ordinary explicit queue synchronization. The
initial DXGI mapping does not assert that an empty queue signal transfers all
display-engine availability to another queue. Windows acceptance must establish
the supported direct-back-buffer cross-queue sequence before advertising that
guarantee. Rendering offscreen on another queue and explicitly handing results to
the presentation queue remains a user composition. No automatic offscreen image,
copy or queue bridge is introduced to make an unsupported sequence appear valid.

## Pacing is user policy

If requested and supported, `chain_pacing_handle` returns a borrowed, typed native
waitable handle. The chain owns and closes the underlying handle; the application
must finish OS waits before resize/destruction and must not close the borrowed handle.
This allows integration with a window/event loop using existing platform waits.
There is no OGPU-specific event-loop or pacing-wait primitive. Unsupported chains
return UNSUPPORTED rather than an always-signaled stand-in.
The native handle is obtained during cold setup, not allocated per frame. Re-query
after a resize attempt; no old-handle validity is promised across reconfiguration.

For the DXGI waitable configuration, the application waits as required by the
native contract, including before its first rendered frame. Pacing is separate
from acquisition and from the user's frame-slot/render timeline. Its notification
is not a per-image acquisition fence, proof that a particular payload can be freed,
or a general scanout-completion API. [Native waitable contract][dx-pacing]

## Present and outcomes

`queue_present` takes explicit frame items, optional retirement fences, explicit
binary waits and caller scratch. DXGI items carry native sync interval/flags;
these fields are zero for the current Vulkan mapping. Rendering, transitions and
ownership transfers are already recorded/submitted by the caller. Native present
may itself block; the wrapper adds no wait, signal, transition or submission.

Preflight the complete batch before any native side effect: identities, generation,
indices, supported queue, requested synchronization, counts, options and scratch.
Array validity/nonoverlap and actual lifetime/pending-state obligations remain the
caller's responsibility. No duplicate chain, binary wait or retirement fence is
allowed. No quadratic duplicate scan or hidden allocation is required.

Retain Vulkan's one-native-call batching. DXGI reports `max_present_count=1`, rejects
binary waits and retirement fences, and requires the queue specified at creation.
A user can loop over windows and own partial success explicitly; the backend must
not hide that loop. Multi-window support does not require native atomic batching.

The mandatory batch outcome reports `work_effect` and `wait_effect` as NONE,
ENQUEUED or UNKNOWN. ENQUEUED says the native operation was accepted, not that an
image was displayed or a wait completed. With no binary waits, `wait_effect=NONE`.
Per-item results report status, frame effect (RETAINED/RELEASED/UNKNOWN), and fence
effect (NONE/ENQUEUED/UNKNOWN). A null retirement fence always has NONE effect.
UNKNOWN forbids inferring retryability, safe reuse or completion. It is not a
successful retirement.

For trusted valid output storage, a local rejection writes NONE batch effects and
leaves per-item results unchanged. After a native call, every item result is
written, including an explicit error/UNKNOWN when native data is unavailable.
The function's aggregate status must preserve failure even if individual items
succeed. The C adapter cannot collapse an error into an OK boolean result.

| Native outcome | Required interpretation |
|---|---|
| Local invalid/unsupported request | Nothing submitted; frames and sync untouched |
| Vulkan OK/SUBOPTIMAL | Work and requested waits/signals enqueued; frame acquisitions released |
| Vulkan out-of-date/surface-lost rejection by presentation engine | Queue operations still enqueued; do not recycle waits or retry the same acquisition as if locally rejected |
| Vulkan native host/device OOM | No resource/sync changes under the native guarantee; frame retained |
| DXGI S_OK | Frame selection released; accepted presentation, no OGPU binary/fence effect |
| DXGI occlusion, retryable native status or unexpected error | Preserve distinct status; classify frame/acceptance only from a documented native guarantee, otherwise UNKNOWN |
| Device loss | Failure; unresolved obligations stay unknown, never turn a completion sentinel into success |

The draft adds OCCLUDED=5 and STILL_DRAWING=6 to the status vocabulary; neither is
an OK alias nor proof of acceptance. Both values are installed SDK vocabulary; the Vulkan mapping does not return them. Native diagnostic detail must also survive. Flags such as TEST/DO_NOT_SEQUENCE
that do not follow ordinary present-and-release semantics are not silently accepted: each needs its
documented effect mapping. This draft selects the outcome vocabulary, not blanket
support for every DXGI flag. [Vulkan queue effects][vk-present], [DXGI results][dx-present]

## Retirement, reuse and abandonment

Rendering completion retires application recordings/ranges according to ordinary
queue completion. It does not alone retire Vulkan presentation's binary waits.
The maintenance presentation fence proves the native payload-retirement condition;
it does not mean display processing is finished or grant image reacquisition.
One signaled fence in a Vulkan batch retires its shared wait semaphores; destroying
a chain also requires its other presentation and independent GPU/acquire uses to
be retired. The application owns that accounting. [Native retirement][vk-fence]

DXGI uses its native swapchain/resource lifetime and queue ordering. No fence is
created or signaled inside present. The application can explicitly place queue
completion points for its GPU work and follow native teardown requirements; OGPU
does not advertise those points as the Vulkan payload-retirement condition.
Windows validation must establish resize/close with pending presentation against
the selected native contract. No scanout-completion capability is claimed.

`abandon` ends unused frame acquisitions without presenting. It accepts frames
from one chain and caller scratch, preserving native Vulkan batched release.
All acquisition/device access must be complete first; it never waits. Vulkan calls
native release, which neither consumes the acquisition binary nor changes layout.
The caller consumes that signal with explicit ordered work or destroys the binary
after its native uses complete. On DXGI it drops the logical selection, without
advancing the native current index or emitting GPU work. Previously recorded work
must not be resubmitted after abandonment. A later acquisition may select that
same image again. [Native release][vk-release]

The abandonment output is RETAINED on preflight rejection, RELEASED on success,
UNKNOWN on an unclassified native failure. A zero-count request is invalid.
Present/abandon do not free the borrowed image wrapper. Wrapper validity and
permission to use its image are different lifetimes.

## Replacement and resize failure

Both strategies are retained because one cannot reproduce the other's lifetime
and scheduling freedom: draining and resizing would lose Vulkan's ability to
present already acquired old images, while pretending DXGI resize creates an
independent chain leaves dangling old-image references.

`chain_create` accepts the existing Vulkan description's optional old chain. Its
outcome always reports the previous chain's state (NONE if absent), as well as the
new chain or null. Preflight rejection preserves previous state. Once native
creation names the old chain, it is RETIRED even if creation or later wrapper setup
fails. Acquired old images remain usable under native rules; unacquired old images
may have lost their backing. Wrapper identity is not permission to access them.
The old wrapper remains until explicit destruction; OGPU keeps no retirement
queue. [Native replacement][vk-create]
An unrecognized description names no decodable previous chain: its result has
NONE/null and no side effects. A recognized, locally invalid description reports
the identified previous chain's unchanged state. Trusted object pointers must
still be valid; this is not recovery from malformed host pointers.

`chain_resize` is capability-dependent in-place reconfiguration. Before calling,
the user ends acquisitions, retires application GPU uses, destroys dependent views,
and resets/destroys recordings and removes external native references to old
images. Descriptor entries referring to old images become unusable. Serialize all
chain/borrow access. These preconditions allow releasing native buffer references;
the operation inserts no drain/wait. [Native resize][dx-resize]

Local rejection reports unchanged generation and IMAGES_UNCHANGED. Once resize
starts releasing wrappers, increment generation and invalidate **all** old borrowed
image pointers, even if the native resize fails. Generation overflow rejects before
side effects. Success returns ACTIVE/IMAGES_REPLACED and newly enumerated wrappers.
Native failure or wrapper-allocation failure returns NO_IMAGES/IMAGES_INVALIDATED
(or LOST after device loss). Do not automatically reconstruct the old wrapper set
and pretend rollback occurred. NO_IMAGES permits explicit resize retry or destroy;
it forbids acquire/present/image enumeration. A retry queries actual native state
as needed; it cannot assume the last requested dimensions were installed. There
is no native rollback guarantee hidden in an unchanged error code.

`chain_info_query` exposes generation/state/image count and associated queue;
NO_IMAGES has count zero. It is a host snapshot, not a native health poll. Device
loss remains a separate error on operations. Enumerated images are borrowed until
invalidation/destruction; dependent views have shorter explicit lifetimes.
Destruction never adds an OGPU wait. Native implementations may incur their native
call cost; neither destruction nor present is promised nonblocking.

## User frame loop and migration

Allocate recording slots and application ranges in user code. Wait/poll only the
slot being reused. Apply native pacing if selected. Acquire a borrowed frame;
record using its actual availability and image-state requirements. Submit work
with caller-selected completion points, present, inspect effects, and track any
native retirement obligation separately. On resize choose the advertised strategy
and its preconditions. For zero-sized/minimized windows, the consumer suspends and
processes events; OGPU does not clamp an extent or busy-loop acquisition.

Each window owns independent chain state; it may share a device, queue, allocator
or application frame ring if the user orders their use. No privileged helper API
is needed. Vulkan's existing three-slot scene and two-window native batch remain
valid migration targets. The DXGI frame loop has no acquisition semaphore or
presentation-fence objects to allocate or recycle.

| Existing surface | Selected migration |
|---|---|
| Wayland setup | Retain; add typed Win32 setup with independently retained native identity |
| Vulkan chain description/mode queries | Retain native semantics; add honest native configuration namespace for DXGI |
| Acquire returning only index | Return borrowed frame with availability and generation; signals remain caller-owned |
| Universal binary/fence requirement | Require only native availability model; no DXGI synthetic queue signals |
| Replacement assumed everywhere | Create-with-old where supported; explicit resize invalidation where supported |
| Present status/optional per-image status | Aggregate effects plus required per-item status/lease/fence effects |
| Universal single native batch | Query maximum batch count; caller composes multiple DXGI presents |
| Release index array | Abandon frame values with explicit effect and reusable native translation scratch |
| Frame pacing in acquire | Borrow native pacing handle where available; event loop stays outside OGPU |
| Generic fence interpreted as present completion | Retain native fence semantics; no fabricated DXGI equivalent |

## Targeted checks and remaining boundaries

C2/Linux: migrate existing Wayland lifecycle and interactive consumers. Test timeout
and suboptimal acquisition, generation/foreign-object rejection, whole-batch local
preflight, known/unknown native effects, old-chain retirement on creation failure,
unused acquisition release and distinct render/present retirement. Reuse existing
two-window and three-slot acceptance; no new benchmark campaign is needed.

Windows handoff: exact supported DXGI flags/statuses, current-buffer ordering and
cross-queue limits, no hidden queue signals/waits, typed HWND ownership after
discovery destruction, waitable-handle ownership, resize failure after old wrapper
release, wrapper allocation failure, retry, and teardown with pending work. These
source-informed decisions do not close review R4/R5/R8 or establish Windows support.

Metal remains deferred: native drawable acquisition/release, timeout policy,
availability and presentation retirement require its native mapping. The common
borrowed-frame concept does not require a permanently enumerable image pool;
a future drawable mapping may use `index=UINT32_MAX` and acquisition-scoped image
lifetime. Neither current mapping advertises that case. Do not emulate Vulkan
objects or promise this draft's lifetime rules already cover Metal.

The [C1 consolidation](minimal-core-migration.md) integrates records, C/Rust
correspondence, platform/query/status namespaces and the deletion map with A/B/D
and dependencies. Runtime migration and native checks remain C2/C3.

## Record check, 2026-11-03

At the P checkpoint, `python3 tools/check-c1-drafts.py` checked A/B/D/P records
together, including imported span and record-header layouts. GCC 16.2.0 and Rust
1.98.1 on Linux x86-64 passed C11/C++17 syntax and 216 C/Rust comparisons.
The checker now covers the consolidated C1 records.
This is schema consistency, not native lifecycle or Windows acceptance.

[dx-chains]: https://learn.microsoft.com/en-us/windows/win32/direct3d12/swap-chains
[dx-pacing]: https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchain2-getframelatencywaitableobject
[dx-present]: https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgiswapchain1-present1
[dx-resize]: https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-resizebuffers
[vk-acquire]: https://docs.vulkan.org/refpages/latest/refpages/source/vkAcquireNextImageKHR.html
[vk-present]: https://docs.vulkan.org/refpages/latest/refpages/source/vkQueuePresentKHR.html
[vk-fence]: https://docs.vulkan.org/refpages/latest/refpages/source/VkSwapchainPresentFenceInfoKHR.html
[vk-release]: https://docs.vulkan.org/refpages/latest/refpages/source/vkReleaseSwapchainImagesKHR.html
[vk-create]: https://docs.vulkan.org/refpages/latest/refpages/source/VkSwapchainCreateInfoKHR.html
