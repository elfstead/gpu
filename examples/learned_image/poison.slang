struct Arguments {
    uint* output_data;
    uint count;
    uint dispatch_width;
};
[[vk::push_constant]] ConstantBuffer<Arguments> root;
[shader("compute")]
[numthreads(64, 1, 1)]
void main(uint3 id : SV_DispatchThreadID) {
    uint i = id.x + id.y * root.dispatch_width;
    if (i < root.count) root.output_data[i] = 0x7fc000a5u;
}
