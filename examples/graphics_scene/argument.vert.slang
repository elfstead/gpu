// The pointer and every payload word affect observable output. No padded dummy root.
#ifndef ARGUMENT_BYTES
#define ARGUMENT_BYTES 8
#endif
struct Root {
    float* vertices;
#if ARGUMENT_BYTES > 8
    uint payload[(ARGUMENT_BYTES - 8) / 4];
#endif
};
[[vk::push_constant]] ConstantBuffer<Root> root;
struct Varyings {
    float4 position : SV_Position;
    nointerpolation uint identity : TEXCOORD0;
};
[shader("vertex")]
Varyings main(uint id : SV_VulkanVertexID, uint draw : SV_DrawIndex) {
    Varyings result;
    result.position = float4(root.vertices[id*4], root.vertices[id*4+1],
                             root.vertices[id*4+2], root.vertices[id*4+3]);
    uint checksum = 0;
#if ARGUMENT_BYTES > 8
    for (uint i=0; i<(ARGUMENT_BYTES-8)/4; ++i)
        checksum += (i+1)*root.payload[i];
#endif
    result.identity = 2*(draw+1) + id/4 + 4*checksum;
    return result;
}
