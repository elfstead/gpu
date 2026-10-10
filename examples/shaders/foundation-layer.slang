struct Root { float4 color; float depth; float rightEdge; uint layer; uint pad; };
struct Control { Root* root; };
[[vk::push_constant]] ConstantBuffer<Control> control;
struct Vertex { float4 position : SV_Position; uint layer : SV_RenderTargetArrayIndex; };
[shader("vertex")]
Vertex vertexMain(uint id : SV_VulkanVertexID) {
    Vertex v;
    float2 p = float2(id == 0 ? -5.0 : control.root->rightEdge, id == 2 ? 5.0 : -5.0);
    v.position = float4(p, control.root->depth, 1.0);
    v.layer = control.root->layer;
    return v;
}
