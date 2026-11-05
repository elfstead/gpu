// Consumer-owned stable compaction: no runtime scheduler or CPU-visible count.
struct Producer { uint* records; uint* count; uint columns; uint pad; };
struct Draw { uint object; uint sequence; uint columns; uint pad; };
[[vk::push_constant]] ConstantBuffer<Producer> producer;
[[vk::push_constant]] ConstantBuffer<Draw> draw;
[[vk::constant_id(9)]] const uint variant = 0;

[shader("compute")]
[numthreads(1, 1, 1)]
void produce() {
    uint count = 0;
    for (uint object = 0; object < 32; ++object) {
        if (object % 8 >= producer.columns) continue;
        uint* r = producer.records + count * 8;
        r[0] = (object + producer.columns) & 1;
        r[1] = object;
        r[2] = producer.columns;
        // r[3] is a guard. Sequence identity comes from the native token.
        r[4] = 6; r[5] = 1; r[6] = r[7] = 0;
        ++count;
    }
    producer.count[0] = count;
}
struct Vertex { float4 position : SV_Position; float4 color : COLOR0; };
[shader("vertex")]
Vertex vertexMain(uint id : SV_VertexID) {
    const float2 corners[6] = {float2(0,0),float2(1,0),float2(0,1),float2(0,1),float2(1,0),float2(1,1)};
    float2 tile = float2(draw.object % 8, draw.object / 8);
    Vertex v;
    v.position = float4((tile + corners[id]) / float2(8,4) * 2 - 1, 0.5, 1);
    v.color = float4(draw.object & 1, variant, (draw.columns + draw.sequence) & 1, 1);
    return v;
}
[shader("fragment")]
float4 fragmentMain(Vertex v) : SV_Target0 { return v.color; }
