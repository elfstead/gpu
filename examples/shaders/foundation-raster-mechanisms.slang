struct Control { float depth; float size; uint unused; uint shape; };
struct Root { Control* control; };
[[vk::push_constant]] ConstantBuffer<Root> root;
struct Vertex { float4 position : SV_Position; nointerpolation float4 color : COLOR0; };
[shader("vertex")]
Vertex vertexMain(uint id : SV_VertexID) {
    Vertex v;float2 p=float2((id<<1)&2,id&2)*2-1;
    if(root.control->shape==1) {
        float2 pixels[3]={float2(4.125,4.125),float2(4.25,4.125),float2(4.125,4.25)};
        p=pixels[id]/4-1;
    }
    if(root.control->shape==4) {
        float2 pixels[3]={float2(3.75,3.75),float2(5.75,3.75),float2(3.75,5.75)};
        p=pixels[id]/4-1;
    }
    if(root.control->shape==2) p=float2(id==0?1.0:7.0,4.5)/4-1;
    v.position=float4(p,root.control->depth,1);
    v.color=root.control->shape==0?float4((17+31*id)/255.0,root.control->size/255.0,37.0/255.0,1):float4(1,1,1,1);
    return v;
}
[shader("fragment")]
float4 fragmentMain(Vertex v) : SV_Target0 { return v.color; }
