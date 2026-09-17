struct Arguments {
    float* input_data;
    float* weights;
    float* output_data;
    uint width;
    uint height;
    uint dispatch_width;
};
[[vk::push_constant]] ConstantBuffer<Arguments> root;
[shader("compute")]
[numthreads(64, 1, 1)]
void main(uint3 id : SV_DispatchThreadID) {
    uint i = id.x + id.y * root.dispatch_width;
    if (i >= root.width * root.height * 8u) return;
    uint channel = i % 8u, pixel = i / 8u;
    int x = int(pixel % root.width), y = int(pixel / root.width);
    precise float value = root.weights[72u + channel];
    for (int ky = -1; ky <= 1; ++ky) for (int kx = -1; kx <= 1; ++kx) {
        int sx = clamp(x + kx, 0, int(root.width) - 1);
        int sy = clamp(y + ky, 0, int(root.height) - 1);
        uint k = channel * 9u + uint((ky + 1) * 3 + kx + 1);
        value += (root.input_data[uint(sy) * root.width + uint(sx)] - 0.5) * root.weights[k];
    }
    root.output_data[i] = max(0.0, value);
}
