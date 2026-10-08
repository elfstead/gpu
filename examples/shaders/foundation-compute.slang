// Pointees are read at execution. Only the root addresses/control bytes are inline.
struct DataRoot { uint* values; uint count; uint add; };
struct Control { DataRoot* source; DataRoot* destination; uint multiplier; uint pad; };
[[vk::push_constant]] ConstantBuffer<Control> control;
[[vk::constant_id(7)]] const uint extra = 1;

[shader("compute")]
[numthreads(64, 1, 1)]
void transform(uint3 id : SV_DispatchThreadID) {
    uint i = id.x;
    if (i < control.source->count && i < control.destination->count) {
        control.destination->values[i] = control.source->values[i] * control.multiplier
            + control.source->add + control.destination->add + extra;
    }
}
