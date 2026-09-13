# D2 follow-up: memory placement and transfers

Approved 2026-09-13. Status: complete; explicit placement/copies adopted experimentally.
Runtime checkpoint: `27f9b6d` (ABI 4).

## Decision and bounds

Compare caller-selected host-accessible/device-local allocations with a runtime
policy that selects placement and hides movement. Use the existing GGML FP32 MNIST
graph: persistent weights, allocator-reused intermediates, input uploads and output
readback. Keep the shaders, numerical gates and direct/scheduled graph modes.

The candidate is explicit placement intent at allocation plus a recorded,
range-checked buffer copy. The runtime selects a compatible Vulkan memory type;
the consumer decides when to stage. No implicit migrations, pointer scanning,
suballocator framework, multiple queues or new benchmark workloads. Host-accessible
does not mean physically separate from device-local memory, especially on UMA.

The alternative remains worth considering even if the explicit path works. Record
whether explicit staging adds useful control or merely burden, and what knowledge
a hidden policy would need (reuse, CPU access frequency and allocation purpose).
GGML marks weight usage after allocation, so that label alone cannot select initial
placement in its allocation callback.

## Acceptance and stopping condition

- Allocation selection tests cover discrete/UMA-like memory properties, unsupported
  placement, and invalid requests. Device-local objects reject CPU access even if
  the driver could map their physical memory.
- Copies validate both devices/ranges and reject overlap; retain both allocations
  through recording, submission and completion destruction. Test discard, failure
  cleanup and copies chained with compute, including partial/unaligned byte ranges.
- Run the existing GGML acceptance under both placements on RADV and llvmpipe.
  Use reusable staging, keep weights/intermediates resident, and verify numerical
  equivalence and existing reuse/lifecycle checks.
- Report transfer bytes/counts and CPU wall time separately from graph execution.
  These synchronous adapter measurements include submission/wait overhead; they
  are not isolated kernel timings or a general performance claim.
- Stop with a justified memory contract, consumer friction report and regression
  evidence. Stabilization and a general allocator are not exit conditions.

## Adopted experimental contract

Replace the experimental allocation signature with an explicit placement argument
(ABI 4), rather than retaining two overlapping creation APIs. HOST supports checked
CPU read/write; DEVICE requires device-local memory and supports GPU use/copy only.
Both own a dedicated allocation and expose a stable non-owning GPU address.
No automatic fallback from DEVICE to non-device-local memory.

Copies take owning buffer handles plus offsets/size, so bounds and lifetime are
checkable. They declare no implicit GPU dependencies: use the existing transfer
access bits and barriers. HOST writes before submission and GPU writes after
completion retain the existing visibility guarantees. Zero-byte copies are checked
no-ops. Non-overlapping ranges of one allocation are legal; overlapping copies are
not memmove. No artificial four-byte restriction is needed for the address command.

Implementation uses the already-required modern address-command extension:
[vkCmdCopyMemoryKHR](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdCopyMemoryKHR.html)
and its [range/usage rules](https://docs.vulkan.org/refpages/latest/refpages/source/VkDeviceMemoryCopyKHR.html).
Internal descriptor-heap allocations remain host-accessible.

## Runtime checkpoint

ABI 4 implemented. All 23 ordinary tests and thirteen opt-in Vulkan tests pass;
the Vulkan tests pass on both RX 5700 XT/RADV and llvmpipe with synchronization
validation enabled. The new test covers odd byte counts/unaligned offsets, guarded
readback, disjoint same-buffer copies, cross-submission copy/compute visibility,
range/device/overlap rejection, DEVICE CPU-access rejection, retained endpoints
after caller ownership is dropped, completion destruction, discard and failed submit.
The C compute example also checks unknown placement rejection and DEVICE CPU-access
errors without modifying read destinations. Device-to-device copies are included.

## Consumer result and policy decision

The adapter selects one placement per session, with DEVICE as the default to
exercise staging and HOST as the direct-access control. This default is not an
automatic performance recommendation. All tensor allocations keep stable addresses;
GGML still owns suballocation and intermediate reuse. One grow-to-fit HOST staging
buffer serves synchronous callbacks, each with a copy batch and wait. The first
weight upload sizes it sufficiently for this workload: no steady-state growth.

The acceptance driver checks exact transfer counts/bytes, not merely predictions.
Each inference performs two uploads (input and test-only logit poisoning) and one
readback. At batch 64, 157 inferences move 31,912,448 upload bytes and 401,920 readback
bytes across the full test set, including padded tail rows. No weight or intermediate
transfers occur during the loop. HOST counts CPU-copy payload, DEVICE GPU-copy
payload: neither measures physical PCIe transactions. Setup includes model loading
and rejection tests and is reported separately, not mislabeled as weight-only cost.

RADV diagnostics, one validated run per placement; sums of adapter CPU wall time
in milliseconds over the existing scheduled full-dataset runs:

| Batch | HOST transfers | HOST graph | DEVICE transfers | DEVICE graph |
|---|---:|---:|---:|---:|
| 1 | 7.465 | 5929.409 | 8341.889 | 4019.105 |
| 17 | 1.243 | 408.045 | 500.966 | 246.925 |
| 64 | 0.779 | 132.521 | 131.634 | 72.591 |

Graph intervals cover recording, submit, wait and resource cleanup; transfer
intervals cover CPU copies and, in DEVICE mode, copy submission/wait/cleanup.
Graph preflight, CPU reference and correctness checks are outside both intervals.
No optional timestamp profile is needed. These are not isolated GPU/kernel timings,
controlled benchmarks or evidence of a generally optimal placement. Validation is
enabled; clock state, run order and driver overhead are not controlled. Direct mode
shows the same qualitative tradeoff in this run. Do not infer a bandwidth number.

The observation is enough for this API decision: moving all data to DEVICE does
not automatically improve this synchronous consumer. Conversely, HOST-only denies
applications the ability to keep long-lived data explicitly device-local.

| Alternative | Decision |
|---|---|
| Caller selects placement; runtime chooses compatible memory type | Adopt. Makes access and movement predictable without exposing Vulkan heaps/types. Caller still chooses ranges, dependencies and reuse. |
| Runtime silently stages every CPU access to DEVICE | Do not adopt in core. This adapter demonstrates that convenience at the consumer layer, including its blocking/copy costs; moving it into the runtime would hide those costs without removing them. |
| Runtime infers placement or migrates allocations | Defer. Size alone lacks reuse/access-frequency information; GGML's weight-usage label arrives after allocation. Relocation also conflicts with already-published raw addresses unless a separate stable-address mechanism is designed. |

The explicit API adds useful control but the adapter does pay for staging ownership,
copy barriers and completion handling. It could batch uploads with compute using
the existing copy command; its synchronous callbacks currently choose not to.
That is a consumer scheduling improvement, not evidence that another host tensor
operator or a general allocator is needed. No such optimization was added here.

HOST/DEVICE is still a candidate, not a final universal taxonomy. A future physical
UMA/BAR consumer may expose a better API alternative combining guaranteed locality
with host access, or separate upload/readback preferences. llvmpipe plus synthetic
memory-property tests do not substitute for that hardware evidence. Keep the simple
contract now; neither freeze it nor add speculative memory classes.

## Reproduction and acceptance

Use the pinned GGML revision and fixture in [the integration instructions](../integrations/ggml/README.md).
For each selected driver, run `bash integrations/ggml/run.sh /path/to/pinned/ggml 0 host`
and the same command ending in `device`, with validation and synchronization
validation enabled. No upstream modifications or shader changes are needed.

Local evidence (ignored logs, not required build inputs):

- RADV HOST: `target/ggml-integration/acceptance.gC5UQgop.log`.
- RADV DEVICE: `target/ggml-integration/acceptance.DCTm6Rlq.log`.
- llvmpipe DEVICE: `target/ggml-integration/acceptance.eg1rDt0U.log`.
- llvmpipe HOST: `target/ggml-integration/acceptance.VsMkPngR.log`.

All six cases in each of these runs pass: CPU-matching top-1 predictions on all
10,000 digits, 9,801 correct, maximum absolute logit error 0.0000343322754, five
dispatches per inference, three scheduled aliases and unchanged allocation reuse.
Lifecycle checks pass, including deliberate live-child rejection; no validation
errors were reported. llvmpipe is software evidence, not a physical integrated GPU.

Final regressions: 23 ordinary tests, all thirteen Vulkan tests on both drivers,
712 C/Rust layout values, clippy with warnings denied, loader mocks, pinned binding
and heap-shader reproduction. The migrated C compute, batch, graphics, image-loop,
heap-image, retirement, reduction and matrix examples all pass on both drivers.
Existing matrix timing output was only a regression check, not new performance
evidence. The bounded checkpoint is complete. Physical UMA/BAR deployment,
asynchronous GGML transfer scheduling and broader allocation policy stay deferred;
none is an unstated prerequisite or an automatically scheduled follow-up.
