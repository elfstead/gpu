struct Coefficients {
    float2 scaleBias;
    uint selectors[2];
};
struct Block {
    float* data;
    Coefficients coefficients[2];
    uint count;
    uint flags;
};
struct Controls {
    float2 gainBias;
    uint mapping[2];
};
struct Arguments {
    Block* blocks;
    Controls controls;
    uint chosen;
    uint epoch;
};
[[vk::push_constant]] ConstantBuffer<Arguments> args;

[shader("compute")]
[numthreads(64, 1, 1)]
void main(uint3 id : SV_DispatchThreadID) {
    Block block = args.blocks[args.chosen];
    if (id.x < block.count) {
        // Push-constant arrays require dynamically uniform indices. Read both
        // constant-indexed values before selecting per invocation; pointed-to
        // coefficient/selector arrays may still use divergent indices.
        uint coefficient = id.x % 2 == 0 ? args.controls.mapping[0] : args.controls.mapping[1];
        Coefficients c = block.coefficients[coefficient];
        block.data[id.x] = block.data[id.x] * c.scaleBias.x + c.scaleBias.y
            + args.controls.gainBias.x * float(c.selectors[id.x % 2])
            + args.controls.gainBias.y + float(block.flags + args.epoch);
    }
}
