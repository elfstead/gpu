struct Root { float* values; float bias; };
struct Control { Root* root; };
[[vk::push_constant]] ConstantBuffer<Control> control;
groupshared float partial;
groupshared float minimum;
groupshared float maximum;

[shader("compute")]
[numthreads(64, 1, 1)]
void bufferAdd(uint3 id : SV_DispatchThreadID) {
    InterlockedAdd(control.root->values[0], float(id.x + 1) * 0.25 + control.root->bias);
}

[shader("compute")]
[numthreads(64, 1, 1)]
void sharedAdd(uint3 id : SV_DispatchThreadID, uint lane : SV_GroupIndex, uint3 group : SV_GroupID) {
    if (lane == 0) partial = 0;
    GroupMemoryBarrierWithGroupSync();
    InterlockedAdd(partial, float(id.x + 1) * 0.25 + control.root->bias);
    GroupMemoryBarrierWithGroupSync();
    if (lane == 0) control.root->values[group.x] = partial;
}

[shader("compute")]
[numthreads(64, 1, 1)]
void bufferMinMax(uint3 id : SV_DispatchThreadID) {
    float value = float(id.x) * 0.25 - 16 + control.root->bias;
    InterlockedMin(control.root->values[0], value);
    InterlockedMax(control.root->values[1], value);
}

[shader("compute")]
[numthreads(64, 1, 1)]
void sharedMinMax(uint3 id : SV_DispatchThreadID, uint lane : SV_GroupIndex, uint3 group : SV_GroupID) {
    if (lane == 0) { minimum = 1024; maximum = -1024; }
    GroupMemoryBarrierWithGroupSync();
    float value = float(id.x) * 0.25 - 16 + control.root->bias;
    InterlockedMin(minimum, value);
    InterlockedMax(maximum, value);
    GroupMemoryBarrierWithGroupSync();
    if (lane == 0) { control.root->values[group.x*2] = minimum; control.root->values[group.x*2+1] = maximum; }
}

[shader("compute")]
[numthreads(64, 1, 1)]
void bufferExchange(uint3 id : SV_DispatchThreadID) {
    float previous;
    InterlockedExchange(control.root->values[id.x], float(id.x) * 0.25 + control.root->bias, previous);
    control.root->values[id.x+128] = previous;
}

[shader("compute")]
[numthreads(64, 1, 1)]
void sharedExchange(uint lane : SV_GroupIndex, uint3 group : SV_GroupID) {
    if (lane == 0) partial = -1024;
    GroupMemoryBarrierWithGroupSync();
    float previous;
    InterlockedExchange(partial, control.root->bias, previous);
    control.root->values[group.x*64+lane] = previous;
    GroupMemoryBarrierWithGroupSync();
    if (lane == 0) control.root->values[128+group.x] = partial;
}
