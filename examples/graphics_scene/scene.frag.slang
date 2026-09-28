[shader("fragment")]
float4 main(nointerpolation uint surface : TEXCOORD0) : SV_Target0 {
    return surface == 0 ? float4(1, 0, 0, 1) : float4(0, 1, 0, 1);
}
