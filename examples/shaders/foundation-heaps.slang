// Heap indices use each descriptor type's aligned size, not a universal slot stride.
struct Control {
    uint source; uint destination; uint samplerIndex; uint output;
    uint width; uint height;
};
[[vk::push_constant]] ConstantBuffer<Control> control;

[shader("compute")]
[numthreads(64, 1, 1)]
void consume(uint3 id : SV_DispatchThreadID) {
    uint i = id.x;
    if (i >= control.width * control.height) return;
    uint2 p = uint2(i % control.width, i / control.width);
    Texture2D<float4> source = ResourceDescriptorHeap[control.source];
    SamplerState filtering = SamplerDescriptorHeap[control.samplerIndex];
    RWTexture2D<float4, 0, 10> destination = ResourceDescriptorHeap[control.destination];
    RWStructuredBuffer<uint> output = ResourceDescriptorHeap[control.output];
    float4 c = source.SampleLevel(filtering, (float2(p) + 0.5) / float2(control.width, control.height), 0);
    float4 value = float4(c.b, 1.0 - c.g, c.r, c.a);
    destination[p] = value;
    uint4 bytes = uint4(round(value * 255.0));
    output[i] = bytes.x | (bytes.y << 8) | (bytes.z << 16) | (bytes.w << 24);
}
