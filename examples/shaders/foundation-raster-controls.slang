struct Control { float depth; float size; uint firstInstance; uint unused; };
struct Root { Control* control; };
[[vk::push_constant]] ConstantBuffer<Root> root;
struct Vertex { float4 position : SV_Position; float4 color : COLOR0; };
[shader("vertex")]
Vertex vertexMain(uint id : SV_VertexID) {
    Vertex v; v.position=float4(float2((id<<1)&2,id&2)*2-1,root.control->depth,1);
    v.color=float4(1,1,1,1);return v;
}
struct Point { float4 position : SV_Position; float size : SV_PointSize; float4 color : COLOR0; };
[shader("vertex")]
Point pointMain() {
    Point v;v.position=float4(0,0,0,1);v.size=root.control->size;v.color=float4(1,1,1,1);return v;
}
[shader("vertex")]
Vertex fetchMain(float4 color : COLOR0,uint id : SV_VertexID,uint instance : SV_VulkanInstanceID) {
    // Four two-pixel-wide strips, each a two-triangle quad. Fetch must advance
    // independently of the actual instance ID used for placement.
    uint corner[6]={0,1,2,2,1,3};uint c=corner[id];
    Vertex v;v.position=float4(-1+0.5*(instance-root.control->firstInstance+(c&1)),(c>>1)*2.0-1,0,1);
    v.color=color;return v;
}
struct Factors { float outer[3] : SV_TessFactor; float inner : SV_InsideTessFactor; };
Factors factors(InputPatch<Vertex,3> patch) { Factors f;f.outer[0]=f.outer[1]=f.outer[2]=f.inner=2;return f; }
[shader("hull")][domain("tri")][partitioning("integer")][outputtopology("triangle_cw")]
[outputcontrolpoints(3)][patchconstantfunc("factors")]
Vertex hullMain(InputPatch<Vertex,3> patch,uint index : SV_OutputControlPointID) { return patch[index]; }
[shader("domain")][domain("tri")]
Vertex domainMain(Factors f,float3 uv : SV_DomainLocation,const OutputPatch<Vertex,3> patch) {
    Vertex v;v.position=patch[0].position*uv.x+patch[1].position*uv.y+patch[2].position*uv.z;
    v.color=patch[0].color;return v;
}
[shader("fragment")]
float4 fragmentMain(Vertex v) : SV_Target0 { return v.color; }
