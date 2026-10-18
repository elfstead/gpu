# DX12 fixture artifacts

`argument-transform.slang` is the ROOT_DESCRIPTORS argument-profile fixture
(see docs/dx12-arguments-profile.md). Its checked-in DXIL artifact is
regenerated with pinned Slang 2026.14.1:

    slangc argument-transform.slang -target dxil -profile cs_6_6 -entry main -o argument-transform.dxil

Tests include the artifact and never provision tools. Regeneration is only
needed when the fixture source changes; keep source and artifact in sync.

`heap-transform.slang` is the descriptor-profile fixture: SM 6.6 direct
descriptor-heap indexing over recipe-materialized descriptors (see
docs/dx12-backend-d2.md). Regenerate with:

    slangc heap-transform.slang -target dxil -profile cs_6_6 -entry main -o heap-transform.dxil

Its argument namespace is 24 bytes (root slot 0 diagnostic buffer, index_in,
index_out, count, reserved); table indices are relative to the bound heap
range start (bind_heap), matching the unbounded descriptor-table artifact ABI
(docs/dx12-backend-d2.md).

`triangle.slang` is the graphics fixture: vertex-id triangle with a constant
fragment color (VS 3336 bytes, PS 2790 bytes). Regenerate with:

    slangc triangle.slang -target dxil -profile vs_6_6 -entry vsMain -o triangle.vs.dxil
    slangc triangle.slang -target dxil -profile ps_6_6 -entry fsMain -o triangle.ps.dxil

Artifacts are checked in exactly as produced (byte-granular sizes; do not pad).
