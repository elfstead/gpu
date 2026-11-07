# M11 native pipeline artifacts

Implemented 2026-11-07 through C and direct Rust. This package is the native
pipeline-binary facility; ray libraries and capture/replay remain separate work.

## Contract and ownership

`PIPELINE_BINARIES` enables individual opaque binary objects. An application can
query the global driver/device identity and a source executable's pipeline key,
probe the native internal cache, create a capture-enabled executable, capture its
ordered binary list, export/import individual key/data pairs, explicitly release
captured pipeline storage, and create an executable from the original ordered
list. Exported storage and deduplication belong to the application. Inputs are
borrowed only during creation and can be destroyed immediately afterwards.

The `EXECUTABLE_BINARIES` extension preserves the existing executable descriptor.
Source code is ignored on this path and can be empty. Stages, entry points,
specialization, argument layout, requirements and static/dynamic state still
describe the identical native executable. Creation uses the supplied binaries;
there is no retry with SPIR-V. Graphics, compute and ray translation share this
input mechanism. Ordinary preparation keeps its existing native path.

Global identity mismatch is rejected before import. Keys compare their declared
length and defined prefix, never undefined tail bytes. Matching identity is not
proof of intact native data or of a matching complete pipeline description. The
caller retains the pipeline-key-to-ordered-binary association and trusted data
provenance. OGPU does not reflect binaries, hash user shaders, validate corrupted
machine code, or invent cross-driver portability. This explicit unsafe contract
matches the other artifact/address boundaries.

Cache-only lookup reports `BINARY_MISSING` and never compiles. Even a previous hit
can later miss; no hit is a reservation. `DISABLE_BINARY_INTERNAL_CACHE` separately
requests the native device-creation policy when supported, and requires
`PIPELINE_BINARIES`. Physical properties distinguish cache existence, control,
preference, prepopulation and compressed binary data. No cache policy is implicit.
Caller-owned aggregate executable caches remain a different native mechanism;
they cannot be combined with binary input or capture. Binary input also forbids
fail-if-compilation-required, matching native valid usage.

Capture count/output follows the native two-call pattern. On failure all created
partial handles are destroyed; no ownership escapes. Returned count is the native
written count, including on CAPACITY, not necessarily required capacity. Re-query
with null output for a new required count. Export CAPACITY instead returns required
byte size and writes no data. Release affects captured native pipeline storage,
not the executable or already-created binaries; repeated release/capture after
release is rejected. All operations are cold and require explicit object lifetime
and host exclusion. Destruction never waits.

## Evidence and remaining acceptance

The independent [Rust consumer](../examples/foundation_rust/src/bin/artifacts.rs)
passes on RX 5700 XT / RADV 26.2.5 with Vulkan synchronization validation:

- Both normal and disabled internal-cache device policies enable successfully.
- Global identity, pipeline keys and native cache hits (one compute binary, two
  graphics binaries) are queried; capture does not change the pipeline key.
- Binary size/export, undersized byte-export preservation, wrong-identity rejection,
  import, explicit capture release and repeated-release rejection pass.
- Compute and two-stage graphics pipelines recreate with empty shader code.
- Imported binaries are destroyed before the compute executables run. Original and
  recreated compute output agree exactly for two inputs, with surrounding guards.
- Graphics recreation is preparation acceptance, not an imported raster-output
  witness. Ray recreation remains without native execution acceptance.

Injected miss/OOM/INCOMPLETE/oversized-count/success results check that partial
objects are destroyed once, no ownership escapes a failure, and no second creation
call or compile fallback occurs. This is cleanup fault injection with a real native
device, not proof that the driver itself returned every injected failure.

An explicit `OGPU_BINARY_CAPACITY_DIAGNOSTIC=1` run asks for only one binary from the
two-binary graphics capture. Both device policies correctly return CAPACITY and
clean the native partial result, but VVL 1.4.357 emits two threading-object
messages per policy. Matching source at
`e4786f7ce8f1319215eff0d938f4be4651cbb85d`,
`layers/thread_tracker/thread_safety_validation.cpp:948`, calls `CreateObject`
only when the result equals `VK_SUCCESS`; the native contract also returns live
handles on `VK_INCOMPLETE`. The object-lifetime tracker accepts nonnegative results
in `object_lifetime_validation.cpp:1951`. This supports a missing threading-tracker
registration diagnosis. The raw diagnostic is separate from normal validation
acceptance; no extra native query, leaked binary, suppressed message or backend
workaround is introduced. Native undersized-capture validation acceptance stays
open until that discrepancy is resolved.

Source-only SDK tooling builds the consumer and offers `--pipeline-artifacts` for
actual execution. Native cache miss correctness is covered by injected status, not
a deterministic local cache miss. Future acceptance also includes imported raster
output, native ray binaries, concurrent caller-managed use and other drivers.

Native references: [proposal](https://docs.vulkan.org/features/latest/features/proposals/VK_KHR_pipeline_binary.html),
[binary input](https://docs.vulkan.org/refpages/latest/refpages/source/VkPipelineBinaryInfoKHR.html),
[partial capture](https://docs.vulkan.org/refpages/latest/refpages/source/VkPipelineBinaryHandlesInfoKHR.html),
[keys](https://docs.vulkan.org/refpages/latest/refpages/source/VkPipelineBinaryKeyKHR.html),
[byte export](https://docs.vulkan.org/refpages/latest/refpages/source/vkGetPipelineBinaryDataKHR.html).

[Receipt](results/m11-pipeline-binaries-2026-11-07.json).
