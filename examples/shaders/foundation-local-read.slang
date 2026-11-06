// Same-pixel attachment and address-backed storage dependencies in one pass.
struct Control { uint inputOffset; uint seed; uint64_t storage; };
[[vk::push_constant]] ConstantBuffer<Control> control;
[[vk::binding(0,0)]] [[vk::input_attachment_index(0)]] SubpassInput<float4> source;
[shader("vertex")]
float4 vertexMain(uint id : SV_VertexID) : SV_Position {
    return float4(float2((id << 1) & 2, id & 2) * 2.0 - 1.0,0,1);
}
[shader("fragment")]
float4 produce(float4 pos : SV_Position) : SV_Target0 {
    uint2 p=uint2(pos.xy); uint i=p.y*8+p.x;
    ((uint*)control.storage)[i]=control.seed+3*i;
    return float4(float(p.x+control.seed)/255.0,float(p.y)/255.0,37.0/255.0,1);
}
[shader("fragment")]
float4 consume(float4 pos : SV_Position) : SV_Target0 {
    uint2 p=uint2(pos.xy); uint v=((uint*)control.storage)[p.y*8+p.x];
    float4 value=source.SubpassLoad();
    return float4(value.b,float(v & 255)/255.0,value.r,1);
}
