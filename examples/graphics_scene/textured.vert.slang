// M4 integrated scene. Both stages intentionally view the same inline bank.
struct Root { float* vertices; uint image; uint samplerIndex; float lod; uint overlay; };
[[vk::push_constant]] ConstantBuffer<Root> root;
struct Varyings {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
    nointerpolation uint surface : TEXCOORD1;
};
[shader("vertex")]
Varyings main(uint id : SV_VulkanVertexID) {
    Varyings result;
    result.position = float4(root.vertices[id * 4], root.vertices[id * 4 + 1],
                             root.vertices[id * 4 + 2], root.vertices[id * 4 + 3]);
    uint corner = id % 4;
    result.uv = float2(corner == 0 || corner == 3 ? 0.0 : 1.0, corner < 2 ? 0.0 : 1.0);
    result.surface = id / 4;
    return result;
}
