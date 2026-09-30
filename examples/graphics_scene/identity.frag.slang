[shader("fragment")]
float4 main(nointerpolation uint identity : TEXCOORD0) : SV_Target0 {
    return float4(float(identity&255u)/255.0,float((identity>>8)&255u)/255.0,1.0,1.0);
}
