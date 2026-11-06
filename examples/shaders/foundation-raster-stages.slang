// Every pre-raster stage reads caller-owned control data through the same root.
struct Control { float vertexValue; float hullValue; float domainValue; float geometryValue; float level; uint mode; };
struct Root { Control* control; };
[[vk::push_constant]] ConstantBuffer<Root> root;
struct Vertex { float4 position : SV_Position; float value : COLOR0; };
[shader("vertex")]
Vertex vertexMain(uint id : SV_VertexID) {
    Vertex v; v.position=float4(float2((id<<1)&2,id&2)*2-1,0,1);
    v.value=root.control->vertexValue; return v;
}
[shader("vertex")]
Vertex adjacencyVertex(uint id : SV_VertexID) { return vertexMain(id/2); }
struct Factors { float outer[3] : SV_TessFactor; float inner : SV_InsideTessFactor; };
Factors factors(InputPatch<Vertex,3> patch) {
    Factors f; f.outer[0]=f.outer[1]=f.outer[2]=f.inner=root.control->level; return f;
}
[shader("hull")]
[domain("tri")][partitioning("integer")][outputtopology("triangle_cw")]
[outputcontrolpoints(3)][patchconstantfunc("factors")]
Vertex hullMain(InputPatch<Vertex,3> patch,uint i : SV_OutputControlPointID) {
    Vertex v=patch[i]; v.value+=root.control->hullValue; return v;
}
[shader("domain")][domain("tri")]
Vertex domainMain(Factors f,float3 uv : SV_DomainLocation,const OutputPatch<Vertex,3> patch) {
    Vertex v; v.position=patch[0].position*uv.x+patch[1].position*uv.y+patch[2].position*uv.z;
    v.value=patch[0].value*uv.x+patch[1].value*uv.y+patch[2].value*uv.z+root.control->domainValue; return v;
}
[shader("geometry")][maxvertexcount(3)]
void geometryMain(triangle Vertex input[3],inout TriangleStream<Vertex> output) {
    for(uint i=0;i<3;++i) { Vertex v=input[i];v.value+=root.control->geometryValue;output.Append(v); }
    output.RestartStrip();
}
[shader("geometry")][maxvertexcount(3)]
void adjacencyMain(triangleadj Vertex input[6],inout TriangleStream<Vertex> output) {
    for(uint i=0;i<3;++i) { Vertex v=input[i*2];v.value+=root.control->geometryValue;output.Append(v); }
    output.RestartStrip();
}
[shader("fragment")]
float4 fragmentMain(Vertex v) : SV_Target0 { return float4(v.value/255.0,float(root.control->mode)/255.0,37.0/255.0,1); }
