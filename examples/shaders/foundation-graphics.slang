struct Root { float4 color; float depth; uint width; uint height; uint pad; };
struct Control { Root* root; };
[[vk::push_constant]] ConstantBuffer<Control> control;
struct Vertex { float4 position : SV_Position; };

[shader("vertex")]
// Native VertexIndex includes base vertex; SV_VertexID subtracts it in Slang.
Vertex vertexMain(uint id : SV_VulkanVertexID) {
    Vertex v;
    float2 p = float2((id << 1) & 2, id & 2);
    v.position = float4(p * 2.0 - 1.0, control.root->depth, 1.0);
    return v;
}
[shader("fragment")]
float4 fragmentMain(Vertex v) : SV_Target0 { return control.root->color; }
