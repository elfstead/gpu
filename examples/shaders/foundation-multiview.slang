// One native draw writes view-dependent depths into a sparse set of array layers.
// Pointee contents change between submissions of the same recorded list.
struct Root { float depth; };
struct Control { Root* root; };
[[vk::push_constant]] ConstantBuffer<Control> control;
[shader("vertex")]
float4 vertexMain(uint id : SV_VulkanVertexID, uint view : SV_ViewID) : SV_Position {
    float2 p = float2(id == 1 ? 3.0 : -1.0, id == 2 ? 3.0 : -1.0);
    return float4(p, control.root->depth + float(view) * 0.125, 1.0);
}
