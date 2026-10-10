struct Root { float4 color; float depth; float rightEdge; uint height; uint pad; float4 secondary; };
struct Control { Root* root; };
[[vk::push_constant]] ConstantBuffer<Control> control;
struct Vertex { float4 position : SV_Position; };
struct Sources {
    [[vk::location(0)]] [[vk::index(0)]] float4 primary : SV_Target0;
    [[vk::location(0)]] [[vk::index(1)]] float4 secondary : SV_Target1;
};
[shader("fragment")]
Sources fragmentMain(Vertex v) {
    Sources s;
    s.primary = control.root->color;
    s.secondary = control.root->secondary;
    return s;
}
