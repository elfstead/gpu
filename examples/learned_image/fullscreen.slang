[shader("vertex")]
float4 main(uint index : SV_VulkanVertexID) : SV_Position {
    // Oversized triangle, with no vertex buffer or push constants.
    float2 p = index == 0 ? float2(-1, -1) : index == 1 ? float2(3, -1) : float2(-1, 3);
    return float4(p, 0, 1);
}
