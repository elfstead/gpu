// Consumer-owned stable compaction: no runtime scheduler or CPU-visible count.
struct Producer { uint* records; uint* count; uint columns; uint action; uint commands; uint pad; };
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
        uint* r = producer.records + count * 16;
        r[0] = (object + producer.columns) & 1;
        r[1] = object;
        r[2] = producer.columns;
        // r[3] is a guard. Sequence identity comes from the native token.
        bool indexed = (producer.action & 1) != 0;
        if (producer.action >= 2) {
            uint* commands = producer.records + 1024 + count*16;
            uint64_t address = (uint64_t)commands;
            r[4] = uint(address); r[5] = uint(address >> 32); r[6] = 32;
            r[7] = producer.commands == 4 ? object%3 : producer.commands;
            for (uint j = 0; j < 2; ++j) {
                uint* d = commands+j*8;
                d[0] = 6; d[1] = 1; d[2] = indexed ? 1+j*6 : 0;
                d[3] = indexed ? uint(-2) : j;
                if (indexed) d[4] = j;
            }
        } else {
            r[4] = 6; r[5] = 1; r[6] = indexed ? 1 : 0;
            r[7] = indexed ? uint(-2) : 0;
            if (indexed) r[8] = 0;
        }
        ++count;
    }
    producer.count[0] = count;
}
struct Vertex { float4 position : SV_Position; float4 color : COLOR0; };
[shader("vertex")]
// Native indices include signed base vertex / first instance. Slang
// SV_VertexID and SV_InstanceID subtract these offsets on SPIR-V targets.
Vertex vertexMain(uint id : SV_VulkanVertexID, uint instance : SV_VulkanInstanceID) {
    const float2 corners[6] = {float2(0,0),float2(1,0),float2(0,1),float2(0,1),float2(1,0),float2(1,1)};
    float2 tile = float2(draw.object % 8, draw.object / 8);
    Vertex v;
    float2 corner = corners[id];
    if (draw.pad != 0) corner.x = (corner.x + instance) * 0.5;
    v.position = float4((tile + corner) / float2(8,4) * 2 - 1, 0.5, 1);
    v.color = float4(draw.object & 1, variant, (draw.columns + draw.sequence) & 1, 1);
    return v;
}
[shader("fragment")]
float4 fragmentMain(Vertex v) : SV_Target0 { return v.color; }
