#include "include/arguments.slangh"
import affine_math;
[[vk::push_constant]] ConstantBuffer<Arguments> args;

[shader("compute")]
[numthreads(LOCAL_SIZE, 1, 1)]
void main(uint3 tid : SV_DispatchThreadID) {
    if (tid.x < args.count)
        args.data[tid.x] = applyAffine(args.data[tid.x], args.scale, args.bias);
}
