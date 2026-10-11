// Two cubes, six axis directions and two explicit view-relative mip levels.
struct Control { uint source; uint samplerIndex; uint* output; };
[[vk::push_constant]] ConstantBuffer<Control> control;
[shader("compute")]
[numthreads(32, 1, 1)]
void cubeMain(uint3 id : SV_DispatchThreadID) {
    uint i = id.x;
    if (i >= 24) return;
    uint face = i % 6;
    float3 direction = float3(0, 0, 0);
    direction[face / 2] = (face & 1) ? -1.0 : 1.0;
    TextureCubeArray<float4> source = ResourceDescriptorHeap[control.source];
    SamplerState sampler = SamplerDescriptorHeap[control.samplerIndex];
    float4 value = source.SampleLevel(sampler, float4(direction, (i / 6) % 2), i / 12);
    uint4 bytes = uint4(round(value * 255.0));
    control.output[i] = bytes.x | (bytes.y << 8) | (bytes.z << 16) | (bytes.w << 24);
}
