struct Root { uint source; uint destination; uint width; uint height; };
[[vk::push_constant]] ConstantBuffer<Root> root;

[shader("compute")]
[numthreads(64, 1, 1)]
void main(uint3 id : SV_DispatchThreadID) {
    uint i = id.x;
    if (i >= root.width * root.height) return;
    uint2 p = uint2(i % root.width, i / root.width);
    Texture2D<float4> source = ResourceDescriptorHeap[root.source];
    // Slang's format type argument 10 is rgba8. A local variable attribute does
    // not specialize a resource dynamically fetched from the heap.
    RWTexture2D<float4, 0, 10> destination = ResourceDescriptorHeap[root.destination];
    float4 c = source.Load(int3(p.x, root.height - 1 - p.y, 0));
    destination[p] = float4(c.b, 1.0 - c.g, c.r, c.a);
}
