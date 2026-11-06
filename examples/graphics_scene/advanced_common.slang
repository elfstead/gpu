// Same bank and stage IO as the existing generated textured vertex/fragment pair.
struct Root { float* vertices; uint image; uint samplerIndex; float lod; uint overlay; };
[[vk::push_constant]] ConstantBuffer<Root> root;
struct Varyings {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
    nointerpolation uint surface : TEXCOORD1;
};
