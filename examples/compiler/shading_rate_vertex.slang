struct Vertex { float4 position : SV_Position; uint rate : SV_ShadingRate; float2 uv : TEXCOORD0; };
[shader("vertex")]
Vertex main(uint id : SV_VulkanVertexID) {
    Vertex v;
    v.position = float4(id == 1 ? 3.0 : -1.0,id == 2 ? 3.0 : -1.0,0,1);
    v.rate = 5; v.uv = float2(0,1); return v;
}
