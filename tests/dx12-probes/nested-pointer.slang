// Probe C: GPU-produced references. An address produced elsewhere (here: read
// from GPU memory) is dereferenced: buffers containing pointers to buffers.
RWStructuredBuffer<uint64_t> table;
RWStructuredBuffer<float> output;

[numthreads(64, 1, 1)]
void main(uint3 tid : SV_DispatchThreadID)
{
    Ptr<float> slot = (Ptr<float>)table[tid.x];
    output[tid.x] = *slot;
}
