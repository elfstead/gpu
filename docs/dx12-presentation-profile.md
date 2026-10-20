# D3 presentation boundary: provisional design for the D3D12 backend

Status: **PROVISIONAL — the reviewer may replace this design.** Laid down
when the D2 boundary closed so a future session implements presentation from
a named plan rather than under pressure. Nothing here is implemented.

## The two constraints that shape it

1. The shared C contract's presentation section is **Wayland-only**
   (`OGPU_NEXT_PRESENTATION_WAYLAND` is the sole kind; `surface_wayland_create`
   the sole constructor; other selections `UNSUPPORTED`).
2. The plan's D3 text: *"Do not implement a fake Vulkan acquire/present
   protocol if DXGI exposes a better native API alternative."*

A Win32 window therefore cannot enter through the shared surface record
today. **Provisional decision (cheap to reverse, quarantined from the shared
API):** the D3D12 `Surface::create` takes the externally supplied window
through the surface record's `surface` pointer with `display` null and
requires the presentation kind to be WAYLAND only as the contract demands;
the record's meaning on this backend is "the native window handle". The
reviewer may instead extend the shared enum with a `PRESENTATION_WIN32` and
a `WIN32_SURFACE_DESC` — that is a shared-header change and is called out
here rather than made unilaterally.

## DXGI mapping (native, not a protocol emulation)

- **Swapchain creation**: `IDXGIFactory2::CreateSwapChainForHwnd` over the
  supplied window; `format`/`color_space` map to the DXGI format and color
  space; `extent`, `min_image_count`, `usage`, `transform`, `alpha`,
  `clipped` map field-for-field. `old_swapchain` maps to `ResizeBuffers` on
  the live chain (DXGI has no swapchain recreation semantics to fake).
- **Images**: `GetBuffer(i)` wrapped as foundation images in `STATE_PRESENT`
  identity; no ownership transfer exists natively and none is invented.
- **Acquire**: DXGI has no acquire. The native model is the current back
  buffer (`IDXGISwapChain3::GetCurrentBackBufferIndex`) plus frame-latency
  objects (`DXGI_FRAME_LATENCY_WAITABLE_OBJECT`) for forward progress. The
  contract's acquire writes the index and schedules the caller's fence/binary
  signals on top of that; `timeout_ns` maps to a bounded wait on the latency
  object, `NOT_READY`/`TIMEOUT` preserve the index.
- **Present**: `IDXGISwapChain3::Present(sync_interval, flags)` lowers the
  whole batch to one native present per chain as the contract requires; the
  per-image fences retire against `Present` completion (`IDXGISwapChain3::
  GetFrameLatencyWaitableObject`/event or the fence on the DXGI queue),
  never against scanout. `SUBOPTIMAL` maps to the format/color-space
  renegotiation signal; `OUT_OF_DATE`/`SURFACE_LOST` map to
  `DXGI_ERROR_INVALID_CALL`/`DXGI_ERROR_ACCESS_LOST` and stay distinct.

## Evidence to gather first (the branch's method)

Probe before implementing, exactly as D1/D2 did:
- does `CreateSwapChainForHwnd` on this AMD driver honor `DXGI_SWAP_CHAIN_FLAG_
  FRAME_LATENCY_WAITABLE_OBJECT` (the acquire/forward-progress mapping);
- the exact status of `Present` after `ResizeBuffers` and after a window
  resize (what maps to `OUT_OF_DATE`);
- whether `GetBuffer` images need explicit state treatment beyond
  `STATE_PRESENT` under enhanced barriers.

## What it unlocks

The M4 scene consumer (`surface --scene-check` and interactive) and every
windowed consumer: their graphics argument work is complete and wired; the
presentation path is their only remaining gate. The consumer also needs a
Win32 window path beside its Wayland one — a consumer-side change named in
`docs/dx12-arguments-profile.md`.
