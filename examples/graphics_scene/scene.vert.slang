struct Root { float* vertices; };
[[vk::push_constant]] ConstantBuffer<Root> root;
struct Varyings {
    float4 position : SV_Position;
    nointerpolation uint surface : TEXCOORD0;
};
[shader("vertex")]
Varyings main(uint id : SV_VulkanVertexID) {
    Varyings result;
    result.position = float4(root.vertices[id * 4], root.vertices[id * 4 + 1],
                             root.vertices[id * 4 + 2], root.vertices[id * 4 + 3]);
    result.surface = id / 4;
    return result;
}
