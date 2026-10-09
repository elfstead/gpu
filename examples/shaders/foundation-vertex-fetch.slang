struct Root { float4 color; float depth; float rightEdge; uint height; uint pad; };
struct Control { Root* root; };
[[vk::push_constant]] ConstantBuffer<Control> control;
struct Vertex { float4 position : SV_Position; };

[shader("vertex")]
Vertex vertexMain([[vk::location(0)]] float2 position : POSITION,
                  [[vk::location(1)]] float2 instance : TEXCOORD0) {
    Vertex v;
    v.position = float4(position, control.root->depth * instance.x + instance.y, 1.0);
    return v;
}
