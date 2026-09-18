struct Arguments {
    float* pixels;
    uint width;
    uint height;
};
[[vk::push_constant]] ConstantBuffer<Arguments> root;
[shader("fragment")]
float4 main(float4 position : SV_Position) : SV_Target0 {
    // Edge helper lanes can execute outside the target. Bound the raw pointer
    // even for lanes whose color will not be written to the framebuffer.
    uint2 p = min(uint2(max(position.xy, float2(0.0))), uint2(root.width - 1u, root.height - 1u));
    uint i = (p.y * root.width + p.x) * 4u;
    return float4(root.pixels[i], root.pixels[i + 1u], root.pixels[i + 2u], root.pixels[i + 3u]);
}
