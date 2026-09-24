struct Arguments
{
    float* data;
    uint count;
    float scale;
    float bias;
};
[[vk::push_constant]] ConstantBuffer<Arguments> args;

[shader("compute")]
[numthreads(64, 1, 1)]
void main(uint3 tid : SV_DispatchThreadID)
{
    if (tid.x < args.count)
        args.data[tid.x] = args.data[tid.x] * args.scale + args.bias;
}
