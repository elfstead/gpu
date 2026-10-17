// Probe B: physical pointer use inside one dispatch. Taking an element
// address, indexing the pointer (pointer arithmetic) and dereferencing is the
// unrestricted pointer strategy the Vulkan backend supports.
RWStructuredBuffer<float> data;

[numthreads(64, 1, 1)]
void main(uint3 tid : SV_DispatchThreadID)
{
    Ptr<float> base = &data[0];
    base[int(tid.x)] = base[int(tid.x)] + 1.0f;
}
