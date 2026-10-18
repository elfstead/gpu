// D3D12 ROOT_DESCRIPTORS argument profile fixture.
//
// The argument namespace is 24 bytes with identical offsets to the Vulkan
// shared-bytes layout:
//   [0..8)   root slot 0 (input address)  - bound as u0, not read as bytes
//   [8..16)  root slot 1 (output address) - bound as u1, not read as bytes
//   [16..20) count
//   [20..24) multiplier
// The cbuffer mirrors the full namespace so byte offsets match across
// backends; the root-slot words are reserved in the constant block.
cbuffer Root : register(b0)
{
    uint2 slot0_bytes;
    uint2 slot1_bytes;
    uint count;
    uint multiplier;
}

RWByteAddressBuffer slot0 : register(u0);
RWByteAddressBuffer slot1 : register(u1);

[numthreads(64, 1, 1)]
void main(uint3 tid : SV_DispatchThreadID)
{
    if (tid.x < count)
    {
        uint value = slot0.Load(tid.x * 4);
        slot1.Store(tid.x * 4, value * multiplier + count);
    }
}
