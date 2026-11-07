# M8 contract consolidation

2026-11-07. The scheduled Vulkan/Linux M8 representation is implemented. Final
installed acceptance is recorded separately below; unavailable hardware and other
native backends remain unaccepted. M9 follows this checkpoint.

## Retained mechanisms and application compositions

| Boundary | Selected mechanism | What stays in the application |
|---|---|---|
| Images/formats | One explicit image/view/copy vocabulary; exact per-format/usage queries; 190 format IDs | Decoding, transcoding, texture streaming, mip policy and format selection |
| Raster/fetch state | Prepared static state and independently enabled native dynamic controls for that same state | Pipeline selection/caching; no OGPU executable permutations |
| Raster stages | Ordinary vertex, geometry, tessellation, task/mesh artifacts and native limits | Subdivision policy, culling and LOD algorithms |
| GPU-generated commands | Explicit token layout, caller-owned streams/counts, preprocessing and scratch | Command compaction, visibility algorithms and scheduling |
| Multiview/shading rate | Native masks, routing and rate inputs with exact capabilities | View grouping, rate-image generation and quality policy |
| Render continuation | Suspend/resume and caller-ordered command lists | Pass partitioning and scheduling |
| Framebuffer-local reads | Explicit attachment indices/output locations, ordinary argument/heap locations and regional dependencies | Subpass graphs, attachment lifetimes, dependency inference and tile policy |
| Geometry capture | Native ordered capture spans, counters/queries and byte-count draws | Stream allocation, overflow policy, serialization and reuse |

Static and dynamic forms do not create two alternative resource or argument
models. A dynamic operation is retained when the native hardware API exposes
independent execution state; replacing it with prepared variants can add pipeline
work and constrain generated execution. Features remain independent; an unsupported
profile does not silently select a shader substitute or extra pass.

The hot paths borrow caller storage or use caller-provided scratch. Cold preparation
may allocate native executable/descriptor metadata. Capture stream query bookkeeping
is reserved explicitly with command arenas. No resource-retention graph, allocator,
frame scheduler or shader reflection enters recording.

## Integrated consumer

The independent advanced textured consumer now has seven explicit profiles and
252 frames at two extents. Its last two profiles combine GPU-produced indexed
arguments, texture heaps/mip views/filtering, depth, blending, scissors,
tessellation and geometry with a color/D32 framebuffer-local read. The second also
suspends/resumes the base render. The local pass rotates color channels and writes
source depth into a color channel, checked against independent color/depth
readback for every pixel. Geometry and continuation preserve the preceding
profile's source images byte-for-byte. Existing CPU texture/depth and generated
geometry/guard checks remain in place.

Each local profile also runs an 8×8 four-sample probe. A per-sample producer writes
different color/depth values. The local shader reads all four samples using
unequal weights, then the attachment resolves. The host checks all 64 pixels and
untouched byte guards. This is bounded native MSAA evidence, not a claim that the
large textured scene uses MSAA or that all local-read combinations pass.

Composition exposed concrete issues:

- LOCAL_READ layout validation previously accepted attachment-only images. The
  shared guard now requires storage usage, or input plus color/depth attachment
  usage. An output that is not read locally uses GENERAL.
- Native render begin resets local attachment mappings even on resume. The
  consumer reissues its mappings; OGPU does not retain or restore shadow state.
- Pinned Slang 2026.14.1 generated invalid Vulkan SPIR-V for scalar
  `SubpassInput<float>`. The validated recipe uses `float4` and `.x` for depth.
  This is an artifact-tool limitation, not a new core resource operation.

The fixed consumer recipe reproduces/validates its advanced-stage and local-read
artifacts and checks reflected root offsets. It is not a general shader linker.
The general generated-interface tool's local-read metadata support stays open.

[Implementation receipt](results/m8-composition-2026-11-07.json) retains the initial
failures as well as the corrected local run. Installed matrix acceptance follows.

## Explicit exclusions and remaining evidence

| Area | Disposition |
|---|---|
| Formats outside the catalog | [Complete pinned inventory](format-inventory.md): plane/media and 64-bit component families assigned to M11; vendor families retain explicit entry criteria. No decompression fallback. |
| Shading rate, nonzero capture rasterization stream, patch restart and other unavailable local profiles | Implemented representation with host/native rejection evidence; positive hardware acceptance remains open. |
| Mesh/task and generated pipeline selection | Existing reported execution/rejection boundaries remain; no new physical mesh or pipeline-selection acceptance. |
| Secondary/multiview/local-read/stencil/capture combinations | Represented by existing mechanisms; unexecuted combinations are evidence debt, not a passing matrix. |
| Metal and DX12 | Foundation mapping/implementation/acceptance remains in each native lane. This receipt proves neither backend. |
| General stage/local-read/capture artifact tooling | Raw native artifacts and bounded recipes are usable. Automatic interface generation for arbitrary combinations remains a tooling task. |
| Primitive-generation query variants and conditional rendering | Not represented by capture counters or ordinary indirect counts. Audit their independent native counter/predicate semantics in the remaining M11 execution/query profiles before claiming complete native execution coverage. |
| Vendor-specific raster facilities outside the selected groups | Not implicitly supported or rejected on principle. A concrete capability/lowering requirement is an entry criterion; M8 is not every Vulkan vendor extension. |

This consolidates the scheduled M8 scope. It does not prove universal performance
parity, complete hardware coverage or API stability. The performance criterion
remains structural: a hypothetical native OGPU implementation must not be forced
into extra work by the public representation. Timing is collected when it can
resolve a specific remaining mechanism question, not for every state setter.
