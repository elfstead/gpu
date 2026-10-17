struct Control { float4 color; };
[[vk::push_constant]] ConstantBuffer<Control> control;
struct Vertex { float4 position : SV_Position; float4 color : COLOR0; };
struct Payload { float4 color; };

Vertex vertex(uint i, float4 color) {
    Vertex v;
    v.position = float4(i == 1 ? 3.0 : -1.0, i == 2 ? 3.0 : -1.0, 0.5, 1.0);
    v.color = color;
    return v;
}
[shader("mesh")]
[numthreads(1, 1, 1)]
[outputtopology("triangle")]
void meshMain(out vertices Vertex vertices[3], out indices uint3 triangles[1]) {
    SetMeshOutputCounts(3, 1);
    for (uint i = 0; i < 3; ++i) vertices[i] = vertex(i, control.color);
    triangles[0] = uint3(0, 1, 2);
}
[shader("amplification")]
[numthreads(1, 1, 1)]
void taskMain() {
    Payload p;
    p.color = control.color.yxzw;
    DispatchMesh(1, 1, 1, p);
}
[shader("mesh")]
[numthreads(1, 1, 1)]
[outputtopology("triangle")]
void payloadMain(in payload Payload p, out vertices Vertex vertices[3], out indices uint3 triangles[1]) {
    SetMeshOutputCounts(3, 1);
    for (uint i = 0; i < 3; ++i) vertices[i] = vertex(i, p.color);
    triangles[0] = uint3(0, 1, 2);
}
[shader("fragment")]
float4 fragmentMain(Vertex v) : SV_Target0 { return v.color; }
