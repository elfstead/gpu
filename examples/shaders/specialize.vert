#version 450
layout(constant_id=0) const float scale = 1.0;
layout(location=0) in vec2 uv;
layout(location=1) in vec2 position;
layout(location=0) out vec2 interpolated;
void main() {
    interpolated = uv;
    gl_Position = vec4(position * scale, 0.0, 1.0);
}
