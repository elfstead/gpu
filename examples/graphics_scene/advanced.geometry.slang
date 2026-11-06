#include "advanced_common.slang"
[shader("geometry")][maxvertexcount(3)]
void main(triangle Varyings vertices[3],inout TriangleStream<Varyings> stream) {
    for(uint i=0;i<3;++i)stream.Append(vertices[i]);
    stream.RestartStrip();
}
