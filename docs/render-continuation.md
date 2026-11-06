# Render-pass continuation

The foundation Vulkan profile exposes continuation through the existing
`render_desc.flags`, render begin/end and ordered submission lists. This completes
the pass-continuation part of M8. Tile-local/input-attachment reads remain separate
implementation work: continuation alone does not authorize an in-render barrier
or an attachment read.

## Contract and native mapping

`RENDER_SUSPENDING` (2) makes render end suspend the pass.
`RENDER_RESUMING` (4) makes render begin resume a previously suspended pass.
Both flags select a middle segment. `RENDER_SECONDARY` (1) continues to select
secondary contents independently. C and direct Rust expose the same bits; record
layouts and foundation revision 11 are unchanged. Exact source-commit matching
remains the SDK contract.

The flags lower directly to `VkRenderingInfo.flags`. The native rule requires
matching rendering parameters across segments except for these three flags.
That includes attachments, layouts, resolve/load/store/clear values, area, layers,
view mask and shading-rate attachment. Host scratch addresses can differ because
scratch is translation storage, not native pass identity. Do not change CLEAR to
LOAD on resume. See the [native rendering flags](https://docs.vulkan.org/refpages/latest/refpages/source/VkRenderingFlagBits.html).

Loads/clears happen at the initial begin. Stores/resolves happen at the final,
non-suspending end. Every begin still needs a matching render end before ending
its command list. Ending a list after a suspending render end is valid. A resumed
list supplies its own executable, arguments, heaps and dynamic state; attachment
continuity does not inherit command-buffer state.

All segments execute in order in **one `submit_desc` batch**. Separate entries in
a single `queue_submit` call remain different native batches. No action or
synchronization command, or unrelated render pass, may execute between suspension
and resumption. State-setting commands are permitted. These constraints follow the
native [rendering](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdBeginRendering.html)
and submission rules; the pinned `validusage.json` contains the corresponding
`VkSubmitInfo2` suspended/resumed-pass requirements.

The caller proves matching parameters and legal composition, including secondary
lists. OGPU checks ordinary local records and rejects unknown flags, but does not
retain copies of pass descriptions, hash attachment identities, maintain a global
continuation registry or attempt to prove submission-wide command contents.
These remain explicit unsafe/native obligations, like resource lifetime, pending
reuse and indirect command contents. Native validation is available for development.

No pass object, hidden load/store, temporary allocation, synchronization edge or
extra submit is added. Callers can record independently and assemble their own
ordered lists. Allocators, parallel recording helpers and render-graph scheduling
remain outside core. Vulkan already supports these bits in the foundation's
native baseline; no new optional feature enable is necessary. This does not claim
that an unported Metal/DX12 implementation accepts the same continuation contract.

## Local evidence and remaining work

The [local receipt](results/m8-render-continuation-2026-11-06.md) covers one-list,
two-list and three-list execution. Each pattern uses both ordinary D32 rendering
and four-sample depth rendering with explicit SAMPLE_ZERO resolve and source
DISCARD. Two replays change depth values through existing argument addresses.
The oracle checks every output value, preserved host roots and guard bytes.
Clearing again at resume would erase earlier draws; discarding a multisample
source at suspension would lose earlier sample values before final resolve.

This is native correctness and structural cost evidence, not a measured bandwidth
or performance claim. Color/stencil, inherited/standalone secondary compositions,
multiview, shading-rate and other-device/backend combinations remain unaccepted.
Their native rules are exposed without claiming that this bounded fixture covers
all of them. Invalid cross-list matching or cross-batch chains are caller errors,
not promised runtime rejection cases.

Next is dynamic-rendering local read: independent feature enabling; attachment
output/input mappings at pipeline, inheritance and recording boundaries; native
local-read layouts; framebuffer-scoped in-render barriers with explicit region/view
semantics; compiler artifacts and a producer/consumer pixel fixture. Existing
input-image descriptors and image usage are reusable. Local-read permissions must
also preserve native storage-buffer/image pixel-local dependencies, rather than
limiting the API to a particular deferred-rendering algorithm.
