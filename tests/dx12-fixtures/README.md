# DX12 fixture artifacts

`argument-transform.slang` is the ROOT_DESCRIPTORS argument-profile fixture
(see docs/dx12-arguments-profile.md). Its checked-in DXIL artifact is
regenerated with pinned Slang 2026.14.1:

    slangc argument-transform.slang -target dxil -profile cs_6_6 -entry main -o argument-transform.dxil

Tests include the artifact and never provision tools. Regeneration is only
needed when the fixture source changes; keep source and artifact in sync.
