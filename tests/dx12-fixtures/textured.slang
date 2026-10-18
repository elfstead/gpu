// D3D12 textured-raster fixture: vertex-buffer inputs (TEXCOORD<N> semantic
// convention: location N binds as TEXCOORD<N>) and descriptor-table sampling.
// The fragment stage snaps UVs to texel centers so pixel expectations are
// exact regardless of sampling rounding.
struct VSIn
{
    float2 position : TEXCOORD0;
    float2 uv : TEXCOORD1;
};

struct VSOut
{
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
};

Texture2D textures[] : register(t0, space1);
SamplerState samplers[] : register(s0, space1);

[shader("vertex")]
VSOut vsMain(VSIn input)
{
    VSOut result;
    result.position = float4(input.position, 0.0, 1.0);
    result.uv = input.uv;
    return result;
}

[shader("fragment")]
float4 fsMain(VSOut input) : SV_Target
{
    float2 texel = floor(input.uv * 4.0);
    float2 snapped = (texel + 0.5) / 4.0;
    return textures[0].Sample(samplers[0], snapped);
}
