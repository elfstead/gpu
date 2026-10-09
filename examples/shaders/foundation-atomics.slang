struct Root { uint64_t* values; };
struct Control { Root* root; };
[[vk::push_constant]] ConstantBuffer<Control> control;
groupshared uint64_t partial;

[shader("compute")]
[numthreads(64, 1, 1)]
void atomicMain(uint3 id : SV_DispatchThreadID, uint lane : SV_GroupIndex, uint3 group : SV_GroupID) {
    if (lane == 0) partial = 0;
    GroupMemoryBarrierWithGroupSync();
    uint64_t value = ((uint64_t(id.x + 1) + control.root->values[3]) << 32) + 3;
    InterlockedAdd(partial, value);
    InterlockedAdd(control.root->values[0], value);
    GroupMemoryBarrierWithGroupSync();
    if (lane == 0) control.root->values[group.x + 1] = partial;
}
