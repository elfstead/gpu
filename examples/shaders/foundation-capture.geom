#version 460
layout(points) in;
layout(points,max_vertices=2) out;
layout(location=0) in vec4 inputPayload[];
layout(location=0,stream=0,xfb_buffer=0,xfb_stride=16,xfb_offset=0) out vec4 payload0;
layout(location=1,stream=1,xfb_buffer=1,xfb_stride=16,xfb_offset=0) out vec4 payload1;
void main() {
    payload0=inputPayload[0]; EmitStreamVertex(0); EndStreamPrimitive(0);
    payload1=inputPayload[0]+vec4(100,0,0,0); EmitStreamVertex(1); EndStreamPrimitive(1);
}
