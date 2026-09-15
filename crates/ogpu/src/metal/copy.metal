#include <metal_stdlib>
using namespace metal;

// OGPU copies are byte-granular. Bind whole buffers at aligned offset zero.
struct Copy { ulong source, destination, count, stride; };
kernel void ogpu_copy_bytes(device const uchar *source [[buffer(0)]],
                           device uchar *destination [[buffer(1)]],
                           constant Copy &p [[buffer(2)]],
                           uint lane [[thread_position_in_grid]]) {
    for (ulong i = lane; i < p.count; i += p.stride)
        destination[p.destination + i] = source[p.source + i];
}
