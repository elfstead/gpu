#ifndef ATOMIC_SCALAR
#define ATOMIC_SCALAR float
#endif
typedef ATOMIC_SCALAR Scalar;
#ifndef ATOMIC_ADD_TERM
#define ATOMIC_ADD_TERM(i) (Scalar((i) + 1) * 0.25)
#endif
struct Root { Scalar* values; Scalar bias; };
struct Control { Root* root; };
[[vk::push_constant]] ConstantBuffer<Control> control;
groupshared Scalar partial;
groupshared Scalar minimum;
groupshared Scalar maximum;

[shader("compute")]
[numthreads(64, 1, 1)]
void bufferAdd(uint3 id : SV_DispatchThreadID) {
    InterlockedAdd(control.root->values[0], ATOMIC_ADD_TERM(id.x) + control.root->bias);
}

[shader("compute")]
[numthreads(64, 1, 1)]
void sharedAdd(uint3 id : SV_DispatchThreadID, uint lane : SV_GroupIndex, uint3 group : SV_GroupID) {
    if (lane == 0) partial = 0;
    GroupMemoryBarrierWithGroupSync();
    InterlockedAdd(partial, ATOMIC_ADD_TERM(id.x) + control.root->bias);
    GroupMemoryBarrierWithGroupSync();
    if (lane == 0) control.root->values[group.x] = partial;
}

[shader("compute")]
[numthreads(64, 1, 1)]
void bufferMinMax(uint3 id : SV_DispatchThreadID) {
    Scalar value = Scalar(id.x) * 0.25 - 16 + control.root->bias;
    InterlockedMin(control.root->values[0], value);
    InterlockedMax(control.root->values[1], value);
}

[shader("compute")]
[numthreads(64, 1, 1)]
void sharedMinMax(uint3 id : SV_DispatchThreadID, uint lane : SV_GroupIndex, uint3 group : SV_GroupID) {
    if (lane == 0) { minimum = 1024; maximum = -1024; }
    GroupMemoryBarrierWithGroupSync();
    Scalar value = Scalar(id.x) * 0.25 - 16 + control.root->bias;
    InterlockedMin(minimum, value);
    InterlockedMax(maximum, value);
    GroupMemoryBarrierWithGroupSync();
    if (lane == 0) { control.root->values[group.x*2] = minimum; control.root->values[group.x*2+1] = maximum; }
}

[shader("compute")]
[numthreads(64, 1, 1)]
void bufferExchange(uint3 id : SV_DispatchThreadID) {
    Scalar previous;
    InterlockedExchange(control.root->values[id.x], Scalar(id.x) * 0.25 + control.root->bias, previous);
    control.root->values[id.x+128] = previous;
}

[shader("compute")]
[numthreads(64, 1, 1)]
void sharedExchange(uint lane : SV_GroupIndex, uint3 group : SV_GroupID) {
    if (lane == 0) partial = -1024;
    GroupMemoryBarrierWithGroupSync();
    Scalar previous;
    InterlockedExchange(partial, control.root->bias, previous);
    control.root->values[group.x*64+lane] = previous;
    GroupMemoryBarrierWithGroupSync();
    if (lane == 0) control.root->values[128+group.x] = partial;
}
