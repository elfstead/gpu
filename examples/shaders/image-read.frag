#version 460
#extension GL_EXT_buffer_reference : require
layout(buffer_reference, std430, buffer_reference_align = 4) readonly buffer Pixels {
    uint values[];
};
layout(push_constant, std430) uniform Root {
    Pixels pixels;
    uint width;
    uint reserved;
} root;
layout(location = 0) out vec4 color;
void main() {
    uvec2 p = uvec2(gl_FragCoord.xy);
    color = unpackUnorm4x8(root.pixels.values[p.y * root.width + p.x]);
}
