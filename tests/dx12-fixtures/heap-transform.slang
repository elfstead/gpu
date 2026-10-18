// D3D12 descriptor-profile fixture: unbounded descriptor-table indexing.
// Argument namespace (24 bytes): [0..8) root slot 0 (diagnostic buffer),
// [8..12) index_in, [12..16) index_out, [16..20) count, [20..24) reserved.
// Table indices are relative to the bound heap range start (bind_heap).
// The diagnostic buffer receives [count, index_in, index_out] before the
// guarded transform through the descriptor table.
cbuffer Root : register(b0)
{
    uint2 slot0_bytes;
    uint index_in;
    uint index_out;
    uint count;
    uint pad;
};
RWByteAddressBuffer diag : register(u0);
RWByteAddressBuffer heap[] : register(u0, space1);

[numthreads(64, 1, 1)]
void main(uint3 tid : SV_DispatchThreadID)
{
    if (tid.x == 0)
    {
        diag.Store(0, count);
        diag.Store(4, index_in);
        diag.Store(8, index_out);
    }
    if (tid.x < count)
    {
        uint v = heap[index_in].Load(tid.x * 4);
        heap[index_out].Store(tid.x * 4, v + 1);
    }
}
