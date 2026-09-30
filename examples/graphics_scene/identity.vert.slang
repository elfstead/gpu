// Deliberately observes command grouping; no extra root or record data.
struct Root { float* vertices; };
[[vk::push_constant]] ConstantBuffer<Root> root;
struct Varyings {
    float4 position : SV_Position;
    nointerpolation uint identity : TEXCOORD0;
};
[shader("vertex")]
Varyings main(uint id : SV_VulkanVertexID, uint draw : SV_DrawIndex) {
    Varyings result;
    result.position=float4(root.vertices[id*4],root.vertices[id*4+1],
                           root.vertices[id*4+2],root.vertices[id*4+3]);
    result.identity=((draw+1)<<1)|(id/4);
    return result;
}
