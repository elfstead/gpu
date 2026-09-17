struct Arguments {
    float* input_data;
    float* weights;
    float* hidden;
    float* output_data;
    uint count;
    uint dispatch_width;
};
[[vk::push_constant]] ConstantBuffer<Arguments> root;
[shader("compute")]
[numthreads(64, 1, 1)]
void main(uint3 id : SV_DispatchThreadID) {
    uint i = id.x + id.y * root.dispatch_width;
    if (i >= root.count) return;
    precise float value = root.weights[88];
    for (uint c = 0; c < 8; ++c) value += root.hidden[i * 8u + c] * root.weights[80u + c];
    root.output_data[i] = clamp(root.input_data[i] + value, 0.0, 1.0);
}
