// D3D12 graphics fixture: vertex-id triangle and constant fragment color.
// The vertex stage consumes no inputs (the artifact's TEXCOORD<N> semantic
// convention is exercised by the vertex-buffer fixture); the triangle covers
// the image center with corners outside the viewport.
struct VSOut
{
    float4 position : SV_Position;
};

[shader("vertex")]
VSOut vsMain(uint vid : SV_VertexID)
{
    VSOut result;
    float2 positions[3] = { float2(0.0, 0.75), float2(-0.75, -0.75), float2(0.75, -0.75) };
    result.position = float4(positions[vid], 0.0, 1.0);
    return result;
}

[shader("fragment")]
float4 fsMain(VSOut input) : SV_Target
{
    return float4(1.0, 0.0, 0.0, 1.0);
}
