struct Control { uint shape; float depth; float slope; uint unused; };
struct Root { Control* control; };
[[vk::push_constant]] ConstantBuffer<Root> root;
struct Vertex { float4 position : SV_Position; };
[shader("vertex")]
Vertex vertexMain(uint id : SV_VertexID) {
    uint corners[6]={0,1,2,2,1,3};
    uint c=root.control->shape==0?corners[id]:id;
    float2 p=root.control->shape==2?float2(id*2.0-1,0.125):float2(c&1,c>>1)*2-1;
    Vertex v;v.position=float4(p,root.control->depth+root.control->slope*(p.x+1)/2,1);return v;
}
[shader("fragment")]
float4 fragmentMain(Vertex v) : SV_Target0 { return float4(1,1,1,1); }
