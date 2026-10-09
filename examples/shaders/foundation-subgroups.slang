struct Result { uint4 ballot; uint sum; uint prefix; uint width; uint lane; };
struct Root { Result* output; uint seed; uint pad; };
struct Control { Root* root; };
[[vk::push_constant]] ConstantBuffer<Control> control;
[shader("compute")]
[numthreads(64, 1, 1)]
void subgroupMain(uint3 id : SV_DispatchThreadID) {
    uint lane = WaveGetLaneIndex();
    uint value = lane + control.root->seed;
    Result r;
    r.ballot = WaveActiveBallot(true);
    r.sum = WaveActiveSum(value);
    r.prefix = WavePrefixSum(value);
    r.width = WaveGetLaneCount();
    r.lane = lane;
    control.root->output[id.x] = r;
}
