[shader("fragment")]
float4 main(uint rate : SV_ShadingRate) : SV_Target { return float4(float(rate)/255.0,0,0,1); }
