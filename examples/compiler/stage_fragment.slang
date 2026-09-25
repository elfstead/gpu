struct Varyings {
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
    nointerpolation uint tag : TEXCOORD1;
};
struct Arguments { uint width; uint height; };
[[vk::push_constant]] ConstantBuffer<Arguments> args;

[shader("fragment")]
float4 main(Varyings input) : SV_Target {
    uint2 p = uint2(input.uv * float2(args.width, args.height));
    uint3 rgb = uint3((p.x * 17 + p.y * 3) & 255,
                     (p.x * 5 + p.y * 29) & 255,
                     (p.x * 11 + p.y * 7) & 255);
    // Both the interpolated coordinate and flat integer must cross the stage
    // interface correctly. Position supplies an independent coordinate check.
    if (input.tag != 17 || any(p != uint2(input.position.xy))) return float4(1, 0, 1, 0);
    return float4(float3(rgb) / 255.0, 1);
}
