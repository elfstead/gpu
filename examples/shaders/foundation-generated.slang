struct Control { uint* output; uint* records; uint* count; uint sequence; uint seed; };
[[vk::push_constant]] ConstantBuffer<Control> control;
[[vk::constant_id(9)]] const uint extra = 100;
[shader("compute")]
[numthreads(1, 1, 1)]
void produce() {
    for (uint i = 0; i < 2; ++i) {
        uint* r = control.records + i*14;
        r[0] = (i + control.seed) & 1;
        r[1] = 0xa5a5a5a5;
        uint64_t output = (uint64_t)control.output;
        r[2] = uint(output); r[3] = uint(output >> 32);
        r[4] = r[5] = r[6] = r[7] = 0;
        r[8] = 0xa5a5a5a5; // Sequence identity comes from a token, not these bytes.
        r[9] = control.seed + i*7;
        r[10] = r[11] = r[12] = 1;
        r[13] = 0xa5a5a5a5;
    }
    control.count[0] = control.seed % 3;
}
[shader("compute")]
[numthreads(1, 1, 1)]
void consume() { control.output[control.sequence] = control.seed + extra; }
