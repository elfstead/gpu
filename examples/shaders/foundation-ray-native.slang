// Reduced native control: no acceleration build or traversal, just a raygen write.
struct Control { uint* output; };
[[vk::push_constant]] ConstantBuffer<Control> control;
[shader("raygeneration")]
void rayMain() {
    uint i = DispatchRaysIndex().x;
    control.output[i] = 0x12340000u + i;
}
