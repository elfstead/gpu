// Prepared heap storage remains live across recording, descriptor updates and replay.
struct Arguments { uint index; uint value; };
[[vk::push_constant]] ConstantBuffer<Arguments> arguments;
[shader("compute")]
[numthreads(1, 1, 1)]
void writeHeap() {
    RWStructuredBuffer<uint> output = ResourceDescriptorHeap[arguments.index];
    output[0] = arguments.value;
}
