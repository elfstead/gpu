// The workflow specializes scalar/format/operation at compile time. Format 0
// means SPIR-V Unknown, not an implicit conversion or typeless native view.
struct Root { uint source; uint destination; uint width; uint height; };
[[vk::push_constant]] ConstantBuffer<Root> args;
[numthreads(8, 1, 1)]
void main(uint3 id : SV_DispatchThreadID) {
    if (id.x >= args.width || id.y >= args.height) return;
    RWTexture2D<TEXEL, 0, INPUT_FORMAT> input = ResourceDescriptorHeap[args.source];
    RWTexture2D<TEXEL, 0, OUTPUT_FORMAT> output = ResourceDescriptorHeap[args.destination];
    TEXEL value = input[id.xy];
#if OPERATION == 0
    output[id.xy] = 1.0 - value;
#elif OPERATION == 1
    output[id.xy] = -value;
#elif OPERATION == 2
    output[id.xy] = value + 17;
#else
    output[id.xy] = value * 2.0 + 1.0;
#endif
}
