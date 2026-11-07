# M11 launch and matrix mapping

Audited 2026-11-07 against the pinned Vulkan registry and native API documentation.
A launch field must describe native control; a compile-time specialization is not
an implementation of a different per-dispatch facility. Unsupported profiles stay
explicit, without a shader rewrite, hidden compilation, extra dispatch or barrier.

| Control | Native mapping | OGPU disposition |
| --- | --- | --- |
| Group counts, direct/indirect | Ordinary Vulkan dispatch/indirect dispatch | Existing direct recording, explicit indirect bytes |
| Workgroup origin | Vulkan `vkCmdDispatchBase`, with pipeline dispatch-base permission for nonzero origins | Implemented `DispatchOrigin` on `Launch.extensions` and `COMPILE_DISPATCH_ORIGIN`; same shader-visible native IDs, one command |
| Fixed local shape and shared arrays | SPIR-V execution modes/types and explicit specialization at executable creation | Existing shader requirements and specialization; changes are cold preparation chosen by the application |
| Per-dispatch local shape | CUDA block dimensions and Metal threads-per-threadgroup | A distinct launch-shape profile is needed on a backend that supports this; current Vulkan SPIR-V pipeline path does not implement it |
| Dynamic workgroup memory | CUDA dynamic shared byte count; Metal indexed threadgroup-memory lengths | Existing scalar `dynamic_shared_bytes` is reserved/unsupported on Vulkan. A scalar alone cannot express Metal's multiple indexed regions; add a counted region extension when implementing that native backend profile |
| Cooperative grid / clusters | CUDA cooperative launch and explicit cluster launch attributes | Separate scheduling/forward-progress contracts, not inferred from a subgroup size, local shape or shared-memory budget. No native mapping in the current Vulkan backend |
| GPU-created work | AMDX execution graphs have native node linkage, scratch requirements/initialization and graph dispatch; ordinary generated commands are a separate mechanism | Explicit optional execution-graph profile still to implement; do not call an indirect dispatch loop equivalent |
| Workgroup/flexible cooperative matrices | NV cooperative-matrix-2 features, tuple granularity, workgroup size and reserved shared-memory properties | Implemented exact fixed/flexible tuple and limit discovery; bounded matrix MLP artifacts built, native local execution unavailable |
| Native binary artifacts | Vulkan pipeline binary import/capture/keys; optional pipeline libraries and capture/replay | Binary capture/import and ray library/handle capture-replay implemented; application owns persistence, compatibility and dependency lifetimes |

[Native Vulkan origins](https://docs.vulkan.org/refpages/latest/refpages/source/vkCmdDispatchBase.html),
[CUDA launch controls](https://docs.nvidia.com/cuda/cuda-driver-api/cuda_driver_api/group__CUDA__EXEC.html),
[Metal 4 compute encoding](https://developer.apple.com/documentation/metal/mtl4computecommandencoder),
[native execution graphs](https://docs.vulkan.org/refpages/latest/refpages/source/VK_AMDX_shader_enqueue.html),
[cooperative-matrix-2](https://docs.vulkan.org/refpages/latest/refpages/source/VK_NV_cooperative_matrix2.html).

## Implemented origin contract

`DispatchOrigin` borrows a three-component base, with no further extension chain.
Each base is strictly below the corresponding maximum group count; each count is
at most maximum minus base. Subtraction validates the sum without overflow.
Nonzero origins require the explicit compute executable flag; zero is legal
without it. The flag is invalid for graphics/ray executable creation. Existing
ordinary dispatches continue to call `vkCmdDispatch`; the extended operation maps
to one `vkCmdDispatchBase`. Indirect launch keeps its native zero-origin contract.

The public Rust shader consumer dispatches a 2×2×1 region at origin (2,1,1), twice
with changed inputs. It checks the four native workgroup IDs, every untouched slot
in the output grid, outer allocation guards, and rejection when the pipeline lacks
the permission. Vulkan synchronization validation is clean. Host checks cover
limit boundaries, zero counts and overflow-sized counts. This is origin acceptance,
not evidence for dynamic memory, grid cooperation or execution graphs.
[Execution receipt](results/m11-dispatch-origin-2026-11-07.json).

## Concrete implementation packages and status

1. Extend matrix discovery with optional workgroup/flexible-dimension profiles and
   exact native numeric types. Keep fixed tuples and granularity/maximum properties
   distinct. Include native reserved workgroup memory in the exposed limits; do not
   silently subtract it from the caller's declared shader memory.
2. Add the bounded ML accelerated variant as an explicit consumer selection. Select
   only an advertised shape/type/scope tuple; compile its specialization visibly.
   Record input conversion, accumulation and layout permissions and compare every
   intermediate against the reference. A host-provided mock tuple may test rejection
   or packing, never successful acceleration. Local matrix hardware is unavailable.
3. Add explicit pipeline binary identity, capture, export and import, then ray
   library/capture-replay variants. The caller controls caches and ownership; a miss
   must not trigger undeclared recompilation or a disk-cache policy.
4. Implement AMDX graph objects as native executable state: caller-owned node
   artifacts/linkage, scratch query and initialization, node-index query, direct and
   indirect graph dispatch. Keep the extension's provisional status visible. The
   core must not parse application graphs, schedule nodes on the CPU, allocate
   scratch implicitly, or replace unavailable support with a persistent kernel.
5. Dynamic local shape, indexed workgroup-memory regions, cooperative-grid and
   cluster profiles remain backend-specific implementation tasks. A future launch
   extension must preserve their native semantics and capability query. Do not
   freeze the present scalar field into a claimed universal contract. Metal/native
   CUDA implementation and acceptance are separate from Vulkan/Linux progress.

Packages 1–3 are now implemented: matrix profiles and the bounded ML consumer
retain local hardware exclusions; binaries have local compute round-trip evidence;
[ray libraries and capture/replay](m11-ray-artifacts.md) have explicit emulation
diagnostics only. Package 4 is next, before plane-aware images and codec sessions.
Package 5 retains separate backend prerequisites. Lack of local hardware is not a
reason to erase a native capability or claim it passed.
