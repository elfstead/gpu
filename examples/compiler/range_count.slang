struct Root { uint* count; uint* active; };
[[vk::push_constant]] ConstantBuffer<Root> root;
[shader("compute")]
[numthreads(1, 1, 1)]
void main(uint3 id : SV_DispatchThreadID) {
    if (id.x == 0) root.count[0] = root.active[0];
}
