struct Root {
    uint8_t* bytes;
    uint16_t* shorts;
    uint64_t* longs;
    double* doubles;
    half* halves;
};
struct Control { Root* root; };
[[vk::push_constant]] ConstantBuffer<Control> control;

[shader("compute")]
[numthreads(64, 1, 1)]
void numericMain(uint3 id : SV_DispatchThreadID) {
    uint i = id.x;
    Root r = *control.root;
    r.bytes[i] = r.bytes[i] + uint8_t(7);
    r.shorts[i] = r.shorts[i] * uint16_t(3) + uint16_t(7);
    uint64_t x = r.longs[i];
    r.longs[i] = (x << 17) ^ (x >> 7) ^ uint64_t(0x123456789abcdef0UL);
    r.doubles[i] = r.doubles[i] * double(0.5) + double(0.0625);
    r.halves[i] = r.halves[i] * half(2.0) + half(0.5);
}
