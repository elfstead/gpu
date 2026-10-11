struct Root { uint* a; uint* b; uint2* output; uint seed; };
struct Control { Root* root; };
[[vk::push_constant]] ConstantBuffer<Control> control;
[shader("compute")]
[numthreads(64, 1, 1)]
void dotMain(uint3 id : SV_DispatchThreadID) {
    Root r = *control.root;
    uint i = id.x;
    uint a = r.a[i], b = r.b[i];
    r.output[i] = uint2(
        asuint(dot4add_i8packed(a, b, asint(0x7fffff00u + i + r.seed))),
        dot4add_u8packed(a, b, 0xffffff00u + i + r.seed));
}
