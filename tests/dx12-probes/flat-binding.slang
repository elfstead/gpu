// Probe A: flat binding baseline. A raw buffer plus uniform data must compile
// for every target; this is the provisional root-constant/root-descriptor
// argument profile's expected artifact shape.
RWByteAddressBuffer data;
uniform uint offset;

[numthreads(64, 1, 1)]
void main(uint3 tid : SV_DispatchThreadID)
{
    uint index = offset + tid.x * 4;
    float value = asfloat(data.Load(index));
    data.Store(index, asuint(value + 1.0f));
}
