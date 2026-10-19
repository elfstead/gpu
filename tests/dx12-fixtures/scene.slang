// D3D12 M4 textured-scene fixture: the argument shape of
// examples/graphics_scene/textured.{vert,frag}.slang lowered for DXIL under
// the ROOT_DESCRIPTORS profile (docs/dx12-arguments-profile.md).
//
// The 24-byte namespace is shared verbatim by both stages at identical
// offsets to the Vulkan shared-bytes layout:
//   [0..8)   root slot 0 (vertices address) - bound as u0, not read as bytes
//   [8..12)  image (descriptor index in the bound resource heap)
//   [12..16) samplerIndex (descriptor index in the bound sampler heap)
//   [16..20) lod
//   [20..24) overlay
// The cbuffer mirrors the full namespace so byte offsets match across
// backends; the root-slot words are reserved in the constant block. The
// vertex pointer lowers to a flat root descriptor (D2.1 pattern); descriptor
// indexing lowers to bound heap tables (D2.2 pattern), where table slot N is
// heap slot N.
cbuffer Root : register(b0)
{
    uint2 slot0_bytes;
    uint image;
    uint samplerIndex;
    float lod;
    uint overlay;
}

RWByteAddressBuffer vertices : register(u0);
Texture2D textures[] : register(t0, space1);
SamplerState samplers[] : register(s0, space1);

struct Varyings
{
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
    nointerpolation uint surface : TEXCOORD1;
};

[shader("vertex")]
Varyings vsMain(uint id : SV_VertexID)
{
    Varyings result;
    result.position = float4(
        asfloat(vertices.Load((id * 4 + 0) * 4)),
        asfloat(vertices.Load((id * 4 + 1) * 4)),
        asfloat(vertices.Load((id * 4 + 2) * 4)),
        asfloat(vertices.Load((id * 4 + 3) * 4)));
    uint corner = id % 4;
    result.uv = float2(corner == 0 || corner == 3 ? 0.0 : 1.0, corner < 2 ? 0.0 : 1.0);
    result.surface = id / 4;
    return result;
}

// The fragment stage consumes the full vertex output struct: D3D12 requires
// the interstage signatures to line up register-for-register (the working
// textured fixture takes the same shape).
[shader("fragment")]
float4 fsMain(Varyings input) : SV_Target0
{
    if (overlay != 0)
    {
        return float4(0.25, 0.5, 1.0, 0.5);
    }
    float3 color = textures[image].SampleLevel(samplers[samplerIndex], input.uv, lod).rgb;
    if (input.surface == 1)
    {
        color *= float3(0.5, 1.0, 0.5);
    }
    return float4(color, 1.0);
}
