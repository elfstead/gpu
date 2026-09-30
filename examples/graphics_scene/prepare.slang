// Consumer-owned native control, not an extension of the installed adapter subset.
struct Root {
    float* vertices;
    uint* indices;
    uint* draws;
    uint phase;
    uint empty;
};
[[vk::push_constant]] ConstantBuffer<Root> root;
uint sceneIndex(uint id) {
    if (id == 2 || id == 5) return 1;
    if (id == 3) return 2;
    if (id == 4 || id == 6) return 3;
    if (id == 7) return 4;
    return 0xffffffffu;
}
[shader("compute")]
[numthreads(8, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint id = thread.x;
    uint corner = id % 4;
    float x = (corner == 0 || corner == 3) ? -1.0 : 1.0;
    float y = corner < 2 ? -1.0 : 1.0;
    bool near = id >= 4;
    root.vertices[4 * id] = near ? x * 0.25 + (root.phase == 0 ? -0.125 : 0.125) : x * 0.75;
    root.vertices[4 * id + 1] = y * (near ? 0.375 : 0.75);
    root.vertices[4 * id + 2] = near ? 0.25 : 0.75;
    root.vertices[4 * id + 3] = 1.0;
    // Two poison entries exercise firstIndex. Native baseVertex is -1 / +3.
#ifdef SCENE_INDEX16
    // Generate pairs with ordinary uint arithmetic/stores. Fixed-function index
    // width does not imply a shader-int16 capability requirement.
    if (id < 4) root.indices[id] = (sceneIndex(2 * id) & 0xffffu) | (sceneIndex(2 * id + 1) << 16);
#else
    root.indices[id] = sceneIndex(id);
#endif
    if (id < 2) {
        root.draws[5 * id] = root.empty != 0 ? 0 : 6;
        root.draws[5 * id + 1] = 1;
        root.draws[5 * id + 2] = 2;
        root.draws[5 * id + 3] = id == 0 ? 0xffffffffu : 3;
        root.draws[5 * id + 4] = 0;
    }
}
