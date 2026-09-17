struct Arguments {
    float* input_data;
    float* output_data;
    uint width;
    uint height;
    uint out_width;
    uint out_height;
    uint dispatch_width;
};
[[vk::push_constant]] ConstantBuffer<Arguments> root;
[shader("compute")]
[numthreads(64, 1, 1)]
void main(uint3 id : SV_DispatchThreadID) {
    uint i = id.x + id.y * root.dispatch_width;
    if (i >= root.out_width * root.out_height) return;
    // Exact source-pixel selection for (pixel+.5)*input/output-.5. The host
    // checks these uint products. Convert only a bounded fraction to FP32;
    // float pixel coordinates amplified rounding errors at video extents.
    uint2 numerator = (2u * uint2(i % root.out_width, i / root.out_width) + 1u)
        * uint2(root.width, root.height);
    uint2 denominator = 2u * uint2(root.out_width, root.out_height);
    int2 base = int2(numerator / denominator);
    uint2 remainder = numerator % denominator;
    uint2 half = uint2(root.out_width, root.out_height);
    // Subtract the half-pixel in integer space too, including border negatives.
    if (remainder.x < half.x) { base.x -= 1; remainder.x += half.x; }
    else remainder.x -= half.x;
    if (remainder.y < half.y) { base.y -= 1; remainder.y += half.y; }
    else remainder.y -= half.y;
    float2 f = float2(remainder) / float2(denominator);
    int2 limit = int2(root.width, root.height) - 1;
    int2 a = clamp(base, int2(0), limit), b = clamp(base + 1, int2(0), limit);
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
