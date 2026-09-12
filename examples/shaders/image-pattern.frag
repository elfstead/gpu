#version 460
layout(location = 0) out vec4 color;
void main() {
    uvec2 p = uvec2(gl_FragCoord.xy);
    uvec3 rgb = uvec3((p.x * 17u + p.y * 3u) & 255u,
                     (p.x * 5u + p.y * 29u) & 255u,
                     (p.x * 11u + p.y * 7u) & 255u);
    color = vec4(vec3(rgb) / 255.0, 1.0);
}
