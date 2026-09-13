# D2 follow-up: memory placement and transfers

Approved 2026-09-13. Status: runtime implemented; consumer comparison in progress.

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

## Candidate contract

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
The C compute example also checks unknown placement rejection.
