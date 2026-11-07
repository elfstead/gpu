struct Control { float depth; float slope; uint shape; uint unused; };
struct Root { Control* control; };
[[vk::push_constant]] ConstantBuffer<Root> root;
struct Vertex { float4 position : SV_Position; };
[shader("vertex")]
Vertex vertexMain(uint id : SV_VertexID) {
    float2 p;
    if(root.control->shape==0) {
        float2 pixels[6]={float2(0,0),float2(4.5,0),float2(0,8),float2(0,8),float2(4.5,0),float2(4.5,8)};
        p=pixels[id]/4-1;
    } else p=float2((id<<1)&2,id&2)*2-1;
    Vertex v;v.position=float4(p,root.control->depth+root.control->slope*(p.x+1)*4,1);return v;
}
[shader("fragment")]
float4 fragmentMain(Vertex v) : SV_Target0 { return float4(1,1,1,1); }
