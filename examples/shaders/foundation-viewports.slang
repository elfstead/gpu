struct Control { float4 colors[2]; uint route; };
struct Root { Control* control; };
[[vk::push_constant]] ConstantBuffer<Root> root;
struct Vertex {
    float4 position : SV_Position;
    nointerpolation float4 color : COLOR0;
    uint viewport : SV_ViewportArrayIndex;
};
[shader("vertex")]
Vertex vertexMain(uint id : SV_VertexID,uint instance : SV_VulkanInstanceID) {
    Vertex v;
    v.position=float4(id==1?3:-1,id==2?3:-1,.5,1);
    v.color=root.control->colors[instance];
    v.viewport=root.control->route!=0?instance:0;
    return v;
}
struct Targets { float4 first : SV_Target0; float4 second : SV_Target1; };
[shader("fragment")]
Targets fragmentMain(Vertex v) { Targets t;t.first=v.color;t.second=v.color.bgra;return t; }
