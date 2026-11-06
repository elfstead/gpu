#include "advanced_common.slang"
struct Factors { float outer[3] : SV_TessFactor; float inner : SV_InsideTessFactor; };
Factors factors(InputPatch<Varyings,3> patch) {
    Factors f;
    f.outer[0]=f.outer[1]=f.outer[2]=f.inner=root.overlay != 0 ? 4 : 2;
    return f;
}
[shader("hull")][domain("tri")][partitioning("integer")][outputtopology("triangle_cw")]
[outputcontrolpoints(3)][patchconstantfunc("factors")]
Varyings main(InputPatch<Varyings,3> patch,uint index : SV_OutputControlPointID) { return patch[index]; }
