struct Control { uint* output; uint seed; uint pad; };
[[vk::push_constant]] ConstantBuffer<Control> control;

[shader("compute")]
[numthreads(64, 1, 1)]
void rotateMain(uint3 id : SV_DispatchThreadID) {
    uint lane = WaveGetLaneIndex();
    uint width = WaveGetLaneCount();
    uint value = lane + control.seed;
    control.output[id.x * 4] = WaveRotate(value, 1);
    control.output[id.x * 4 + 1] = WaveClusteredRotate(value, 1, 4);
    control.output[id.x * 4 + 2] = lane;
    control.output[id.x * 4 + 3] = width;
}

[shader("compute")]
[DerivativeGroupLinear]
[numthreads(64, 1, 1)]
void derivativeLinear(uint3 id : SV_DispatchThreadID) {
    float value = float(id.x + control.seed);
    control.output[id.x * 2] = asuint(ddx_fine(value));
    control.output[id.x * 2 + 1] = asuint(ddy_fine(value));
}

[shader("compute")]
[DerivativeGroupQuad]
[numthreads(8, 8, 1)]
void derivativeQuad(uint3 id : SV_DispatchThreadID) {
    float value = float(id.x + id.y * 8 + control.seed);
    uint index = id.x + id.y * 8;
    control.output[index * 2] = asuint(ddx_fine(value));
    control.output[index * 2 + 1] = asuint(ddy_fine(value));
}

struct NarrowControl { uint* output; uint8_t small; uint8_t pad; uint16_t medium; };
[[vk::push_constant]] ConstantBuffer<NarrowControl> narrow;
[shader("compute")]
[numthreads(64, 1, 1)]
void narrowPush(uint3 id : SV_DispatchThreadID) {
    narrow.output[id.x] = uint(narrow.small) + uint(narrow.medium) * 256 + id.x;
}
