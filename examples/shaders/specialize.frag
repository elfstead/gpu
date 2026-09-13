#version 450
layout(constant_id=0) const float scale = 1.0;
layout(location=0) in vec2 interpolated;
layout(location=0) out vec4 color;
void main() { color = vec4(scale, interpolated, 1.0); }
