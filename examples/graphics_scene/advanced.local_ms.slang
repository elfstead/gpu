struct LocalRoot { uint color; uint depth; };
[[vk::push_constant]] ConstantBuffer<LocalRoot> root;
[[vk::binding(0,0)]] [[vk::input_attachment_index(0)]] SubpassInputMS<float4> sourceColor;
[[vk::binding(1,0)]] [[vk::input_attachment_index(1)]] SubpassInputMS<float4> sourceDepth;
struct SampleOutput { float4 color : SV_Target0; float depth : SV_Depth; };
[shader("fragment")]
SampleOutput localProduce(float4 pos : SV_Position,uint sample : SV_SampleIndex) {
    SampleOutput o;
    o.color=float4(float(sample+1)/8.0,float(uint(pos.x))/16.0,float(uint(pos.y))/16.0,1);
    o.depth=float(sample+1)/8.0;return o;
}
[shader("fragment")]
float4 localConsumeMS() : SV_Target0 {
    float4 color=0;float depth=0;
    [unroll] for(uint i=0;i<4;++i) {
        float4 c=sourceColor.SubpassLoad(int(i));
        color+=float4(c.r*float(i+1)/10.0,c.gb/4.0,0);
        depth+=sourceDepth.SubpassLoad(int(i)).x*float(4-i)/10.0;
    }
    return float4(color.gb,depth,color.r);
}
