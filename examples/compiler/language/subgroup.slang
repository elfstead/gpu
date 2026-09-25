struct Arguments { uint* input_data; uint* output_data; uint count; };
[[vk::push_constant]] ConstantBuffer<Arguments> root;

[shader("compute")]
[numthreads(64, 1, 1)]
void main(uint3 id : SV_DispatchThreadID) {
    // All launched invocations participate, including padding. A minimum global
    // ID identifies each subgroup without assuming contiguous invocation mapping.
    uint value = id.x < root.count ? root.input_data[id.x] : 0;
    uint sum = WaveActiveSum(value);
    uint leader = WaveActiveMin(id.x);
    uint active = WaveActiveSum(1u);
    root.output_data[id.x * 4] = sum;
    root.output_data[id.x * 4 + 1] = leader;
    root.output_data[id.x * 4 + 2] = active;
    root.output_data[id.x * 4 + 3] = WaveGetLaneIndex();
}
