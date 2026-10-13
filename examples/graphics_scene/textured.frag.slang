struct Root { float* vertices; uint image; uint samplerIndex; float lod; uint overlay; };
[[vk::push_constant]] ConstantBuffer<Root> root;
struct Inputs { float2 uv : TEXCOORD0; nointerpolation uint surface : TEXCOORD1; };
[shader("fragment")]
float4 main(Inputs input) : SV_Target0 {
    if (root.overlay != 0) return float4(0.25, 0.5, 1.0, 0.5);
    Texture2D<float4> image = ResourceDescriptorHeap[root.image];
    SamplerState sampler = SamplerDescriptorHeap[root.samplerIndex];
    float3 color = image.SampleLevel(sampler, input.uv, root.lod).rgb;
    if (input.surface == 1) color *= float3(0.5, 1.0, 0.5);
    return float4(color, 1.0);
}
