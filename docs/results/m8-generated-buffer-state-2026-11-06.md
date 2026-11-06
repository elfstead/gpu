# M8: generated vertex/index bindings and explicit vertex stride

2026-11-06, following `9532bab`. Foundation **revision 8** adds native generated
vertex/index binding tokens and extends the existing vertex-binding operation
with explicit stride state. C and direct Rust share the same implementation.
No additional host operation, shader vertex-fetch emulation, generated-record
translation or command-time allocation is introduced.

## Contract and migration from revision 7

- `GeneratedToken` / `ogpu_next_generated_token` keeps its four-u32 layout, with
  `argument_offset` renamed `target` and `argument_size` renamed `size`. For
  argument/sequence tokens these retain their byte meanings. A vertex-buffer
  token uses `target` as its binding number and `size=0`; an index-buffer token
  requires both zero. Duplicate vertex destinations and duplicate index tokens
  reject. Both token kinds are limited to vertex graphics, not compute/mesh.
- New 16-byte wire records contain address/u32 size/u32 stride for vertices and
  address/u32 size/u32 index encoding for indices. Addresses and execution-time
  accesses are trusted caller inputs using VERTEX/INDEX memory with pending-use
  lifetimes. The driver consumes the GPU records directly.
- `GeneratedLimits` adds `index_u16` and `index_u32`. Producers use these queried
  values in index wire records; they are **not** host `INDEX_U16/INDEX_U32` enums.
  Vulkan selects its supported native input mode, preferring Vulkan index values;
  DXGI-mode values can be used directly when that is the advertised native mode.
  A future DX12 backend can supply its own native encodings through the same query.
  This does not implement or accept a DX12 backend.
- `bind_vertices` now accepts `VertexBufferBinding { span, stride, flags }` records
  through both C and Rust. `VERTEX_BIND_STRIDE` sets a dynamic stride and requires
  a consuming `DYNAMIC_VERTEX_STRIDE` pipeline. Flags zero requires stride zero
  and retains static pipeline strides. Rebind explicitly when changing strategies.
  Old static callers replace each span with `{span, 0, 0}`. No parallel old API
  or pipeline-dependent command rewriting is retained.
- Generated vertex tokens require dynamic-stride pipelines and declared target
  bindings. The mandatory cold requirements query validates that pairing against
  the fixed/initial executable. Set members retain the existing compatible-state
  obligation. Cold layout/executable metadata records the binding numbers; the
  recording path does not scan them.
- Generated index tokens need no prior host index binding. After execution they
  invalidate OGPU's known index range, even for zero generated sequences. Subsequent
  ordinary indexed work must rebind explicitly. Generated vertex bindings/strides
  must likewise be re-established before later work relies on them. No implicit
  state restoration or GPU-count observation is added.

The native wire forms are specified by
[Vulkan vertex binding commands](https://docs.vulkan.org/refpages/latest/refpages/source/VkBindVertexBufferIndirectCommandEXT.html)
and [index-token input modes](https://docs.vulkan.org/refpages/latest/refpages/source/VkIndirectCommandsIndexBufferTokenEXT.html).
The Vulkan DXGI input mode accepts D3D12's compatible index-view layout;
[DXGI encodings](https://learn.microsoft.com/en-us/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format)
for R16_UINT/R32_UINT are 57/42. Those encoding choices remain backend metadata,
not an alternative public record or a universal shader constant.

One validation finding matters to the API: address-based Vulkan vertex binding
explicitly distinguishes setting a dynamic stride from using static pipeline
stride. Always setting stride is invalid for a static-stride pipeline. The binding
flag preserves that native choice in one operation rather than hiding state changes
in the implementation. It also preserves static preparation as a native strategy;
this checkpoint makes no speed ranking between static and dynamic state.

## Verification

Radeon RX 5700 XT, RADV/Mesa 26.2.5, Vulkan and synchronization validation enabled:

- All **28 native GPU library tests** pass. The public C fixture passes without
  validation errors; existing unrelated shader-output warnings remain.
- Generated graphics runs both the previous profile and the new state-token
  profile: **96 submissions, four action forms, two preprocessing modes and
  196,608 pixels** pass, with whole 32 KiB allocation comparisons per submission.
- The new profile has no host vertex/index prebind. GPU records choose per-object
  vertex addresses at sparse binding 3 and alternating 8/16-byte strides plus
  16/32-bit index views.
  Ordinary/indexed/nested-count actions retain signed base vertex, first instance,
  zero counts, count clamping and separate/integrated preprocessing coverage.
- Cold rejection checks preserve outputs for static-stride and undeclared-binding
  executable pairings. Rejected recordings prove index-range invalidation after
  generated execution, including zero work. CPU tests reject duplicate binding
  destinations, wrong stages and malformed token payloads. Ordinary vertex-fetch
  fixtures exercise static and dynamic stride; oversized stride rejects.
- **79 CPU library tests**, two direct Rust correspondence tests, strict Clippy,
  formatting, eight installer tests and two surface-parser tests pass. Shader
  artifacts reproduce with Slang 2026.14.1 and pass SPIR-V validation.
- The installed-surface audit checks **151 signatures, 126 records and 80 constants
  (1,084 values)**. Combined native/legacy/foundation ABI checks cover 2,043 values.
  SDK revision checks and in-tree consumers move together to foundation 8.

Reproduce using `cargo test --offline --no-default-features -p ogpu`, the ignored
GPU tests with `--test-threads=1`, `cargo xtask foundation`,
`cargo xtask foundation-shaders --check`, `cargo xtask abi` and
`python3 tools/check-foundation-surface.py`. [Clean installed-SDK acceptance](m8-revision8-sdk-2026-11-06.md) now passes at
`b5414e5`, including all selected C, GGML/libplacebo and isolated Rust/Wayland consumers.

## Remaining work

This closes the initial vertex/index-state token implementation and local execution
gap. Graphics pipeline selection, generated mesh, Windows and Metal execution
remain unaccepted locally. The Vulkan DXGI input-mode branch is not exercised by
this Radeon run. No second physical GPU, universal performance parity or stable ABI
is claimed. M8 continues with advanced raster profiles, starting with multiview;
shading-rate, tile-local/pass-continuation and remaining tessellation/geometry
profiles remain scheduled coverage. Larger scene and external-consumer work remain
separate from the bounded fixture.
