#include <metal_stdlib>
using namespace metal;
struct Root { device uint *output; uint value; };
kernel void fill(constant Root &root [[buffer(0)]], uint id [[thread_position_in_grid]]) {
    root.output[id] = root.value + id;
}
