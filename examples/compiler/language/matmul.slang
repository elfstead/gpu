struct Arguments {
    float* a; float* b; float* c;
    uint m; uint n; uint k; uint lda; uint ldb; uint ldc;
};
[[vk::push_constant]] ConstantBuffer<Arguments> root;
groupshared float a_tile[64];
groupshared float b_tile[64];

[shader("compute")]
[numthreads(64, 1, 1)]
void main(uint3 local : SV_GroupThreadID, uint3 group : SV_GroupID) {
    uint lane = local.x;
    uint y = lane / 8;
    uint x = lane % 8;
    uint column_tiles = root.n / 8 + uint(root.n % 8 != 0);
    uint row = (group.x / column_tiles) * 8 + y;
    uint col = (group.x % column_tiles) * 8 + x;
    float sum = 0;
    for (uint base = 0; base < root.k; base += 8) {
        a_tile[lane] = row < root.m && base + x < root.k ? root.a[row * root.lda + base + x] : 0;
        b_tile[lane] = base + y < root.k && col < root.n ? root.b[(base + y) * root.ldb + col] : 0;
        GroupMemoryBarrierWithGroupSync();
        for (uint j = 0; j < 8; ++j) sum += a_tile[y * 8 + j] * b_tile[j * 8 + x];
        GroupMemoryBarrierWithGroupSync();
    }
    if (row < root.m && col < root.n) root.c[row * root.ldc + col] = sum;
}
