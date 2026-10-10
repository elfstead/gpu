// Attachmentless rasterization with address-root storage and explicit stage enabling.
struct Root { uint* output; uint seed; uint pad; };
struct Control { Root* root; };
[[vk::push_constant]] ConstantBuffer<Control> control;
struct Vertex { float4 position : SV_Position; };
Vertex geometry(uint id) {
    Vertex v;
    v.position = float4(id == 1 ? 3.0 : -1.0, id == 2 ? 3.0 : -1.0, 0.5, 1.0);
    return v;
}
[shader("vertex")]
Vertex vertexRead(uint id : SV_VulkanVertexID) { return geometry(id); }
[shader("vertex")]
Vertex vertexMain(uint id : SV_VulkanVertexID) {
    // Vertex invocation counts may exceed the logical index count. OR is idempotent;
    // neither an exactly-once assumption nor a non-atomic duplicate write is needed.
    uint ignored;
    InterlockedOr(control.root->output[0], 1u << id, ignored);
    InterlockedOr(control.root->output[4 + id], (id + 1) << control.root->seed, ignored);
    return geometry(id);
}
[shader("fragment")]
void fragmentRead(Vertex v) {}
[shader("fragment")]
void fragmentMain(Vertex v) {
    uint2 pixel = uint2(v.position.xy);
    uint index = pixel.y * 8 + pixel.x;
    uint value = index + 1 + control.root->seed * 17;
    control.root->output[16 + index] = value;
    uint ignored;
    InterlockedAdd(control.root->output[1], value, ignored);
}
