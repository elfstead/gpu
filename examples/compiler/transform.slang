// Same integer transform as examples/compute.c; the shader owns its interface.
struct Arguments {
    uint* data;
    uint count;
};
[[vk::push_constant]] ConstantBuffer<Arguments> args;

[shader("compute")]
[numthreads(64, 1, 1)]
void main(uint3 id : SV_DispatchThreadID) {
    if (id.x < args.count) args.data[id.x] = args.data[id.x] * 3u + 7u;
}
