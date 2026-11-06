#include "advanced_common.slang"
struct Factors { float outer[3] : SV_TessFactor; float inner : SV_InsideTessFactor; };
[shader("domain")][domain("tri")]
Varyings main(Factors factors,float3 weights : SV_DomainLocation,const OutputPatch<Varyings,3> patch) {
    Varyings v;
    v.position=patch[0].position*weights.x+patch[1].position*weights.y+patch[2].position*weights.z;
    v.uv=patch[0].uv*weights.x+patch[1].uv*weights.y+patch[2].uv*weights.z;
    v.surface=patch[0].surface;
    return v;
}
