struct Arguments {
    float* input_data;
    float* output_data;
    uint width;
    uint height;
    uint out_width;
    uint out_height;
};
[[vk::push_constant]] ConstantBuffer<Arguments> root;
[shader("compute")]
[numthreads(64, 1, 1)]
void main(uint3 id : SV_DispatchThreadID) {
    uint i = id.x;
    if (i >= root.out_width * root.out_height) return;
    float2 p = (float2(i % root.out_width, i / root.out_width) + 0.5)
        * float2(root.width, root.height) / float2(root.out_width, root.out_height) - 0.5;
    int2 base = int2(floor(p)), limit = int2(root.width, root.height) - 1;
    int2 a = clamp(base, int2(0), limit), b = clamp(base + 1, int2(0), limit);
    float2 f = p - floor(p);
    precise float top = root.input_data[uint(a.y) * root.width + uint(a.x)] * (1.0 - f.x)
        + root.input_data[uint(a.y) * root.width + uint(b.x)] * f.x;
    precise float bottom = root.input_data[uint(b.y) * root.width + uint(a.x)] * (1.0 - f.x)
        + root.input_data[uint(b.y) * root.width + uint(b.x)] * f.x;
    precise float gray = top * (1.0 - f.y) + bottom * f.y;
    root.output_data[i * 4u] = 0.95 * gray;
    root.output_data[i * 4u + 1u] = 0.8 * gray + 0.05;
    root.output_data[i * 4u + 2u] = 0.6 * gray + 0.15;
    root.output_data[i * 4u + 3u] = 1.0;
}
