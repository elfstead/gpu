struct Varyings {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
    nointerpolation uint tag : TEXCOORD1;
};

[shader("vertex")]
Varyings main(uint id : SV_VulkanVertexID) {
    float2 uv = float2((id << 1) & 2, id & 2);
    Varyings result;
    result.position = float4(uv * 2.0 - 1.0, 0, 1);
    result.uv = uv;
    result.tag = 17 + id / 3;
    return result;
}
