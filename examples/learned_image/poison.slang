struct Arguments {
    uint* output_data;
    uint count;
    uint reserved;
};
[[vk::push_constant]] ConstantBuffer<Arguments> root;
[shader("compute")]
[numthreads(64, 1, 1)]
void main(uint3 id : SV_DispatchThreadID) {
    if (id.x < root.count) root.output_data[id.x] = 0x7fc000a5u;
}
