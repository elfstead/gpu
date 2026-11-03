// C2 prepared arguments: each buffer declaration has an explicit native mapping.
// Source coordinates deliberately differ from logical push-data offsets.
struct InlineValues { uint count; uint multiplier; };
struct UniformValues { uint add; };
[[vk::binding(3, 2)]] ConstantBuffer<InlineValues> values;
[[vk::binding(0, 5)]] ConstantBuffer<UniformValues> uniformValues;
[[vk::binding(4, 3)]] StructuredBuffer<uint> source;
[[vk::binding(1, 7)]] RWStructuredBuffer<uint> destination;

// Direct physical-pointer bits can coexist with the buffer mappings. Padding
// places this field at logical byte 32; it does not add a second update mechanism.
struct DirectValues { uint4 unused0; uint4 unused1; uint* extra; };
[[vk::push_constant]] ConstantBuffer<DirectValues> direct;
[[vk::constant_id(7)]] const uint bias = 1;

[shader("compute")]
[numthreads(32, 1, 1)]
void mappedArguments(uint3 id : SV_DispatchThreadID) {
    if (id.x < values.count)
        destination[id.x] = source[id.x] * values.multiplier + uniformValues.add + *direct.extra + bias;
}
