// Fixed consumer pass: same-pixel color and depth reads inside dynamic rendering.
struct LocalRoot { uint color; uint depth; };
[[vk::push_constant]] ConstantBuffer<LocalRoot> root;
[[vk::binding(0,0)]] [[vk::input_attachment_index(0)]] SubpassInput<float4> sourceColor;
[[vk::binding(1,0)]] [[vk::input_attachment_index(1)]] SubpassInput<float4> sourceDepth;
[shader("vertex")]
float4 localVertex(uint id : SV_VertexID) : SV_Position {
    return float4(float2((id << 1) & 2,id & 2)*2-1,0,1);
}
[shader("fragment")]
float4 localFragment() : SV_Target0 {
    float4 c=sourceColor.SubpassLoad();
    float d=sourceDepth.SubpassLoad().x;
    return float4(c.b,c.r,d,1);
}
