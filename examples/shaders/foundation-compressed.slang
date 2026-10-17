// Native compressed sampling through the ordinary descriptor heap. Copy tests
// use encoded blocks directly; this shader does not decompress in software.
struct Control {
    uint source; uint samplerIndex; uint* output;
    uint width; uint height; uint mip; uint padding;
};
[[vk::push_constant]] ConstantBuffer<Control> control;
[shader("compute")]
[numthreads(64, 1, 1)]
void compressedMain(uint3 id : SV_DispatchThreadID) {
    uint i = id.x;
    uint pixels = control.width * control.height;
    if (i >= pixels * 2) return;
    uint p = i % pixels;
    Texture2DArray<float4> source = ResourceDescriptorHeap[control.source];
    SamplerState filtering = SamplerDescriptorHeap[control.samplerIndex];
    float2 uv = (float2(p % control.width, p / control.width) + 0.5)
        / float2(control.width, control.height);
    float4 value = source.SampleLevel(filtering, float3(uv, i / pixels), control.mip);
    uint4 bytes = uint4(round(value * 255.0));
    control.output[i] = bytes.x | (bytes.y << 8) | (bytes.z << 16) | (bytes.w << 24);
}
