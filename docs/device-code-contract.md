# Device-code contract — experimental, ABI 17

This is the current programming boundary and the starting point for
[M2](compiler-contract-plan.md), not a new source-language specification. The
[C header](../include/ogpu.h) owns host-call rules; compiled code and its checked
interface own shader layout and execution requirements. Native Metal is a separate
artifact/backend path; Vulkan compiler success does not validate it.

## Responsibilities

| Layer | Owns | Does not infer |
|---|---|---|
| Runtime/API | Buffers/images/heaps, enabled capabilities and limits, copied root bytes, commands, explicit dependencies, completion and retained objects | Tensor shapes, reachable pointer graphs, aliasing, logical dispatch bounds, numerical error budgets |
| Offline compiler adapter | Entry/stage, local size, artifact bytes, checked root/pointee layout and mapped requirements for its declared subset | Allocation, uploads, pointer lifetime, barriers, dispatch policy or application semantics |
| Application/device code | Extents, indexing, alignment, reachable storage, racing accesses, algorithm and numerical permissions | Safety from an arbitrary address merely because it was put in a generated struct |

Shader input is trusted, not sandboxed. `spirv-val` and metadata checks are necessary
but do not prove address bounds, race freedom or algorithm correctness. Invalid
shader accesses are outside the contract; the runtime does not promise to diagnose
them or make them safe. Generated helpers are a convenience, not the only way to
produce a compatible executable.

## Memory and addresses

HOST and DEVICE are allocation/access policies, not source-level pointer types.
A GPU buffer address is a device-local non-owning 64-bit value, represented as
`uint64_t` in generated C, never a dereferenceable CPU pointer. A borrowed HOST
view is a distinct CPU address. Neither can be exchanged for the other even on
unified memory. Arithmetic on GPU addresses must stay within live storage and
respect the final accessed type's extent/alignment. No cross-device address use.

Inline root bytes are copied when a command is recorded; native executable creation
declares their size. Pointer targets are not copied or retained by that copy. Keep
all transitively reachable allocations alive, or explicitly retain each relevant
buffer. A parameter block may itself hold other addresses: retaining the block
does not retain its pointees. Replay fixes roots/commands while allowing pointed-to
data to change under the synchronization contract.

Private/function storage belongs to invocations; workgroup storage belongs to one
workgroup; buffer-address storage is backed by application allocations. Device
code must not escape invocation/workgroup pointers into a persistent address graph.
M2's generated layout support does not imply generation support for every storage
class. The existing scalar adapter rejects workgroup/global resources even though
hand-built reduction/matrix executables already use workgroup memory.

## Scalar and aggregate representation

Generated layouts currently target the tested little-endian host/device workflow.
They are not a wire format. Widths are explicit: 32-bit unsigned fields and 64-bit
GPU addresses; FP32 scalar roots are M2's first extension. FP32 pointees already
work. Native C `bool`, `long`, enums, pointers and compiler vector extensions are
not substitute layout declarations. Half storage and half arithmetic are separate
capabilities; neither implies arbitrary half fields in generated roots.

Every emitted aggregate must verify field offsets, alignment, size and array stride
against compiler output. C arrays of scalars may represent a shader vector only
with explicitly verified layout; `float3` is not assumed to have any particular
alignment. Include internal/trailing padding and C static assertions. Slang type
layout is context-dependent, and size can differ from array stride; recursive
generation must carry the layout context. [Slang reflection](https://docs.shader-slang.org/en/stable/external/slang/docs/user-guide/09-reflection.html)

For physical-address loads/stores, respect the alignment encoded in the actual
SPIR-V operation, including stronger vectorized accesses, not merely an informal
source-type minimum. Arbitrary byte offsets can invalidate that promise.
[Vulkan address alignment](https://docs.vulkan.org/guide/latest/buffer_device_address_alignment.html)

Unsupported or unverifiable layouts must reject. Do not silently repack at every
dispatch, truncate fields or require all argument blocks to be inline. Whether the
current reflection JSON adequately describes pointed-to aggregates is an explicit
M2 check; a name alone is insufficient evidence of a layout.

## Aliasing, ordering and work distribution

The host API does not imply `restrict`/no-alias for distinct root fields. Multiple
addresses may refer to the same allocation when the emitted shader's aliasing
rules and the application algorithm permit it. Compiler assumptions still apply;
a generated interface does not repeal native alias decorations or source-language
rules. Any proposed no-alias optimization must be explicit and tested in M2.

Dispatch dimensions count workgroups; the executable supplies fixed local XYZ
dimensions. Applications derive the logical grid and guard excess invocations.
No fixed subgroup size, workgroup execution order or cross-workgroup progress
guarantee is promised. Do not implement a global barrier by assuming all workgroups
are simultaneously resident.

Memory visibility is not execution rendezvous. Workgroup cooperation needs the
appropriate device-code memory/control barriers, reached with the required uniform
participation. Host batch barriers and split endpoints order commands; they cannot
repair a missing barrier inside a dispatch. Likewise an atomic operation is not an
implicit publication barrier for unrelated non-atomic memory. Slang exposes distinct
memory-only and group-synchronizing operations. [Slang barriers](https://docs.shader-slang.org/en/latest/external/core-module-reference/global-decls/barrier.html)

The current Vulkan scalar adapter validates `GLSL450` memory-model output with
Logical or PhysicalStorageBuffer64 addressing. It does not currently accept an
arbitrary memory model, subgroup profile or atomic extension. M2 must map actual
requirements to enabled support before accepting those artifacts; physical-device
support alone is not enablement. Unknown requirements reject, not silently fall back.

CPU copy helpers require whole-buffer completion; borrowed HOST views permit
independent aligned ranges with explicit flush/invalidate as applicable. GPU-to-GPU
dependencies remain explicit even after host completion observation. Recording order
alone is not a memory dependency. Split endpoints select prefix/suffix scopes;
intervening independent work is not included merely for sharing a pipeline stage.

## Numerical permissions

Representation, storage conversion and arithmetic precision are separate choices.
An FP32 field does not promise bit-identical evaluation across compilers/devices,
particular contraction/reassociation, denormal handling or transcendental accuracy.
Record compiler flags, emitted capabilities/modes and algorithm-specific error
requirements. Optional FP16 arithmetic needs the enabled capability; storage-only
FP16 weights do not grant that permission. Native float-control modes impose their
own requirements. [Vulkan SPIR-V environment](https://docs.vulkan.org/spec/latest/appendices/spirvenv.html)

Existing learned-image and matrix reference tolerances remain authoritative for
those workloads. The first FP32-root fixture instead uses exactly representable
values to isolate byte transport/layout. Wider numerical conformance, accelerated
matrix profiles and a project-wide fast-math policy remain undecided.

## Graphics and resources

Compute, vertex and fragment executables have different stage inputs/outputs; a
root type alone is not a stage interface. The currently generated graphics subset
is rootless Vulkan vertex index to position, and fragment coordinates to location-0
float4 color with a flat root. General varyings, attributes and heap shaders are
M2 extensions, not support implied by this document.

Image/sampler heap indices are non-owning values in distinct heaps. Applications
must establish descriptor kind, valid slot/range, dimensions, format, sampling and
storage permissions, and obey heap mutation/retention rules. Buffers remain physical
addresses rather than implicitly converted descriptors. The runtime owns backend
image layout handling; shader helpers must not add hidden copies or representation
changes. Graphics resources and ML buffers participate in the same explicit memory
and dependency model, without implying that today's narrow raster API is complete.

## Open decisions and progression

M2 tests aggregate/pointee layouts, generated resource/stage interfaces, declared
source dependencies and compiler output for workgroup/subgroup/matrix/graphics
idioms. It ends with a language-direction record, not automatic Slang adoption.
Runtime restrictions and adapter restrictions are tracked separately. The standing
performance gate asks whether a native implementation can preserve native Vulkan
opportunities; a convenient generator must not impose compulsory copies, synchronization
or repeated host interpretation that a native interface can avoid.
