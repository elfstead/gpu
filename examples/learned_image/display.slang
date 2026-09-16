struct Arguments {
    float* pixels;
    uint width;
    uint reserved;
};
[[vk::push_constant]] ConstantBuffer<Arguments> root;
[shader("fragment")]
float4 main(float4 position : SV_Position) : SV_Target0 {
    uint2 p = uint2(position.xy);
    uint i = (p.y * root.width + p.x) * 4u;
    return float4(root.pixels[i], root.pixels[i + 1u], root.pixels[i + 2u], root.pixels[i + 3u]);
}
