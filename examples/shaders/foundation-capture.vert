#version 460
#extension GL_EXT_buffer_reference2 : require
// Pinned native-mechanism fixture. Slang's GLSL frontend rejects xfb qualifiers.
layout(buffer_reference, std430, buffer_reference_align=4) readonly buffer Control { uint seed; };
layout(push_constant) uniform Root { Control control; } root;
#ifdef NO_CAPTURE
layout(location=0) out vec4 payload;
#else
layout(location=0, xfb_buffer=0, xfb_stride=16, xfb_offset=0) out vec4 payload;
#endif
void main() {
    payload=vec4(float(root.control.seed+gl_VertexIndex),float(gl_VertexIndex*2),3.0,4.0);
    gl_Position=vec4(0,0,0,1); gl_PointSize=1.0;
}
