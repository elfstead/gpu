struct Root { uint image; uint samplerIndex; uint width; uint height; float offsetX; };
[[vk::push_constant]] ConstantBuffer<Root> root;

[shader("fragment")]
float4 main(float4 position : SV_Position) : SV_Target {
    Texture2D<float4> image = ResourceDescriptorHeap[root.image];
    SamplerState sampler = SamplerDescriptorHeap[root.samplerIndex];
    return image.SampleLevel(sampler, (position.xy + float2(root.offsetX, 0)) / float2(root.width, root.height), 0);
}
