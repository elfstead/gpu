#version 460
#extension GL_EXT_buffer_reference : require

layout(buffer_reference, std430, buffer_reference_align = 16) readonly buffer Vertices {
    vec4 positions[];
};
layout(push_constant, std430) uniform Root {
    Vertices vertices;
} root;

void main() {
    gl_Position = root.vertices.positions[gl_VertexIndex];
}
