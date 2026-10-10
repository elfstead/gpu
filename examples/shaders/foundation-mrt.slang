struct Root { float4 color; float depth; float rightEdge; uint height; uint pad; };
struct Control { Root* root; };
[[vk::push_constant]] ConstantBuffer<Control> control;
struct Vertex { float4 position : SV_Position; };
struct Targets { float4 first : SV_Target0; float4 second : SV_Target1; };
[shader("fragment")]
Targets fragmentMain(Vertex v) {
    Targets t;
    t.first = control.root->color;
    t.second = control.root->color.bgra;
    return t;
}
