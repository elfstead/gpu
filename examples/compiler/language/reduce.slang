struct Arguments { uint* input_data; uint* output_data; uint count; };
[[vk::push_constant]] ConstantBuffer<Arguments> root;
groupshared uint partial[64];

[shader("compute")]
[numthreads(64, 1, 1)]
void main(uint3 local : SV_GroupThreadID, uint3 group : SV_GroupID) {
    uint lane = local.x;
    uint first = group.x * 128 + lane;
    uint second = first + 64;
    uint value = 0;
    if (first < root.count) value += root.input_data[first];
    if (second < root.count) value += root.input_data[second];
    partial[lane] = value;
    GroupMemoryBarrierWithGroupSync();
    for (uint stride = 32; stride != 0; stride >>= 1) {
        if (lane < stride) partial[lane] += partial[lane + stride];
        GroupMemoryBarrierWithGroupSync();
    }
    if (lane == 0) root.output_data[group.x] = partial[0];
}
