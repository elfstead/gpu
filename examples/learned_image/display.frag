#version 460
#extension GL_EXT_buffer_reference : require
layout(buffer_reference, std430, buffer_reference_align = 4) readonly buffer Pixels { float v[]; };
layout(push_constant, std430) uniform Root { Pixels pixels; uint width; uint reserved; } root;
layout(location = 0) out vec4 color;
void main() {
    uvec2 p = uvec2(gl_FragCoord.xy);
    uint i = (p.y * root.width + p.x) * 4u;
    color = vec4(root.pixels.v[i], root.pixels.v[i + 1u], root.pixels.v[i + 2u], root.pixels.v[i + 3u]);
}
