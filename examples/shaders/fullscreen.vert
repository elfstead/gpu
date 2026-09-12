#version 460
void main() {
    // An oversized triangle covers every sample without an internal shared edge.
    const vec2 positions[3] = vec2[3](vec2(-1, -1), vec2(3, -1), vec2(-1, 3));
    gl_Position = vec4(positions[gl_VertexIndex], 0, 1);
}
