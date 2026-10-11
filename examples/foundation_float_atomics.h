/* Native FP16/FP32/FP64 operations, independent memory domains. Exact binary fractions make
 * the finite-data oracle order-independent without promising arbitrary FP order. */
#define FLOAT_ATOMIC_FEATURES (OGPU_NEXT_FEATURE_BUFFER_FLOAT32_ATOMICS | OGPU_NEXT_FEATURE_BUFFER_FLOAT32_ATOMIC_ADD | OGPU_NEXT_FEATURE_BUFFER_FLOAT32_ATOMIC_MIN_MAX | OGPU_NEXT_FEATURE_SHARED_FLOAT32_ATOMICS | OGPU_NEXT_FEATURE_SHARED_FLOAT32_ATOMIC_ADD | OGPU_NEXT_FEATURE_SHARED_FLOAT32_ATOMIC_MIN_MAX)
#define FLOAT64_ATOMIC_FEATURES (OGPU_NEXT_FEATURE_BUFFER_FLOAT64_ATOMICS | OGPU_NEXT_FEATURE_BUFFER_FLOAT64_ATOMIC_ADD | OGPU_NEXT_FEATURE_BUFFER_FLOAT64_ATOMIC_MIN_MAX | OGPU_NEXT_FEATURE_SHARED_FLOAT64_ATOMICS | OGPU_NEXT_FEATURE_SHARED_FLOAT64_ATOMIC_ADD | OGPU_NEXT_FEATURE_SHARED_FLOAT64_ATOMIC_MIN_MAX)
#define FLOAT16_ATOMIC_FEATURES (OGPU_NEXT_FEATURE_BUFFER_FLOAT16_ATOMICS | OGPU_NEXT_FEATURE_BUFFER_FLOAT16_ATOMIC_ADD | OGPU_NEXT_FEATURE_BUFFER_FLOAT16_ATOMIC_MIN_MAX | OGPU_NEXT_FEATURE_SHARED_FLOAT16_ATOMICS | OGPU_NEXT_FEATURE_SHARED_FLOAT16_ATOMIC_ADD | OGPU_NEXT_FEATURE_SHARED_FLOAT16_ATOMIC_MIN_MAX)
/* Host fixture constants must be exactly representable, finite normal halves or
 * zero. Reject accidental oracle narrowing; this is not a runtime conversion API. */
static uint16_t float_atomic_half(double value) {
    float f = (float)value; uint32_t u; memcpy(&u,&f,sizeof(u));
    uint32_t exponent = (u >> 23) & 255u;
    if (value != 0 && ((double)f != value || exponent < 113 || exponent > 142 || (u & 8191u))) {
        fprintf(stderr,"Non-exact half atomic fixture constant.\n"); abort();
    }
    return (uint16_t)((u >> 16 & 0x8000u) | (value == 0 ? 0 : ((exponent-112) << 10) | ((u >> 13) & 1023u)));
}
static double float_atomic_read(const void *values, uint32_t index, uint32_t bits) {
    if (bits == 64) return ((const double *)values)[index];
    if (bits == 32) return ((const float *)values)[index];
    uint16_t h = ((const uint16_t *)values)[index];
    uint32_t sign = (uint32_t)(h & 0x8000u) << 16, mantissa = h & 1023u, exponent = (h >> 10) & 31u, u;
    if (exponent == 31) u = sign | 0x7f800000u | (mantissa << 13);
    else if (exponent) u = sign | ((exponent+112) << 23) | (mantissa << 13);
    else if (!mantissa) u = sign;
    else {
        exponent = 113;
        while (!(mantissa & 1024u)) { mantissa <<= 1; --exponent; }
        u = sign | (exponent << 23) | ((mantissa & 1023u) << 13);
    }
    float f; memcpy(&f,&u,sizeof(f)); return f;
}
static void float_atomic_write(void *values, uint32_t index, uint32_t bits, double value) {
    if (bits == 64) ((double *)values)[index] = value;
    else if (bits == 32) ((float *)values)[index] = (float)value;
    else ((uint16_t *)values)[index] = float_atomic_half(value);
}
static int float_atomic_execution(ogpu_next_device *device, ogpu_next_memory_desc md, uint32_t domain,
                                  const char *path, uint32_t mode, uint32_t bits) {
    int result = EXIT_FAILURE, pending = 0;
    FILE *file = NULL; void *code = NULL, *scratch = NULL;
    ogpu_next_executable *executable = NULL; ogpu_next_memory *memory = NULL;
    ogpu_next_arena *arena = NULL; ogpu_next_timeline *done = NULL;
    const uint64_t required[] = {OGPU_NEXT_FEATURE_BUFFER_FLOAT32_ATOMIC_ADD,OGPU_NEXT_FEATURE_SHARED_FLOAT32_ATOMIC_ADD,
        OGPU_NEXT_FEATURE_BUFFER_FLOAT32_ATOMIC_MIN_MAX,OGPU_NEXT_FEATURE_SHARED_FLOAT32_ATOMIC_MIN_MAX,
        OGPU_NEXT_FEATURE_BUFFER_FLOAT32_ATOMICS,OGPU_NEXT_FEATURE_SHARED_FLOAT32_ATOMICS};
    const char *entries[] = {"bufferAdd","sharedAdd","bufferMinMax","sharedMinMax","bufferExchange","sharedExchange"};
    const uint32_t counts[] = {1,2,2,4,256,130};
    const uint32_t shared[] = {0,4,0,8,0,4};
    REQUIRE(mode < 6 && (bits == 16 || bits == 32 || bits == 64));
    if (bits == 16) {
        /* Exercise host conversion even when this driver has no half atomics. */
        for (int i = -512; i <= 512; ++i) {
            double exact = i*0.125; uint16_t h = float_atomic_half(exact);
            REQUIRE(float_atomic_read(&h,0,16) == exact);
        }
        const uint16_t special[] = {1,1023,1024,0x7bff,0x8000,0x7c00,0xfc00,0x7e00};
        REQUIRE(float_atomic_read(special,0,16) == 0x1p-24);
        REQUIRE(float_atomic_read(special,1,16) == 1023*0x1p-24);
        REQUIRE(float_atomic_read(special,2,16) == 0x1p-14);
        REQUIRE(float_atomic_read(special,3,16) == 65504);
        REQUIRE(float_atomic_read(special,4,16) == 0);
        REQUIRE(float_atomic_read(special,5,16) > 65504 && float_atomic_read(special,6,16) < -65504);
        double nan = float_atomic_read(special,7,16); REQUIRE(nan != nan);
    }
    uint32_t width = bits / 8;
    uint64_t needed = bits == 64 ? (required[mode] << 6) | OGPU_NEXT_FEATURE_FLOAT64
        : bits == 16 ? (required[mode] << 16) | OGPU_NEXT_FEATURE_FLOAT16 : required[mode];
    ogpu_next_feature_info features = {0}; ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));
    TRY(ogpu_next_device_query(device,&q));
    if ((features.enabled & needed) != needed) { printf("Float%u atomic %s NOT exercised: feature unavailable.\n",width*8,entries[mode]); return EXIT_SUCCESS; }
    file = fopen(path,"rb"); REQUIRE(file != NULL);
    REQUIRE(fseek(file,0,SEEK_END) == 0); long size = ftell(file); REQUIRE(size >= 20 && size % 4 == 0);
    REQUIRE(fseek(file,0,SEEK_SET) == 0); code = malloc((size_t)size); REQUIRE(code != NULL);
    REQUIRE(fread(code,1,(size_t)size,file) == (size_t)size); fclose(file); file = NULL;
    ogpu_next_root_slot slot = {OGPU_NEXT_STAGE_COMPUTE,0,8};
    ogpu_next_argument_interface abi = {HEADER(ogpu_next_argument_interface,OGPU_NEXT_ARGUMENT_INTERFACE),8,1,&slot};
    ogpu_next_shader_requirements req = {HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),needed,{64,1,1},shared[mode]*width/4};
    ogpu_next_shader shader = {OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_SHADER_SPIRV,{code,(size_t)size},entries[mode],&abi.header,NULL,NULL};
    ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_COMPUTE,
        1,&shader,NULL,0,&req.header,NULL,0,0};
    TRY(ogpu_next_executable_create(device,&ed,&executable)); free(code); code = NULL;
    TRY(ogpu_next_memory_create(device,&md,&memory));
    ogpu_next_span all = {memory,0,md.size}; ogpu_next_mapping map = {0}; TRY(ogpu_next_memory_map(all,&map));
    uint64_t address = 0; TRY(ogpu_next_memory_address(all,&address));
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena));
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    ogpu_next_encoder *encoder = NULL; ogpu_next_list *list = NULL; TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    ogpu_next_bind_executable(encoder,executable); ogpu_next_set_root(encoder,OGPU_NEXT_STAGE_COMPUTE,0,address);
    ogpu_next_launch launch = {{2,1,1},0,NULL}; ogpu_next_dispatch(encoder,&launch);
    ogpu_next_dependency dep = {HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_STAGE_HOST,
        OGPU_NEXT_ACCESS_SHADER_WRITE,OGPU_NEXT_ACCESS_HOST_READ,0,0,0,NULL,NULL,NULL,0};
    ogpu_next_barrier(encoder,&dep); TRY(ogpu_next_commands_end(encoder,&list));
    TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_host_requirements sr = {0}; TRY(ogpu_next_submit_scratch_requirements(1,0,1,&sr));
    REQUIRE(sr.alignment <= _Alignof(max_align_t)); scratch = malloc((size_t)sr.size); REQUIRE(scratch != NULL);
    ogpu_next_sync_point signal = {{done,1},OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {HEADER(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),1,0,1,&list,NULL,&signal,scratch,sr.size};
    for (uint32_t replay = 0; replay < 2; ++replay) {
        memset(map.data,0xa5,(size_t)md.size);
        double bias = (replay ? -0.25 : 0.5) + (bits == 64 ? 0x1p-30 : 0);
        unsigned char root[16] = {0}; uint64_t output = address+1024;
        memcpy(root,&output,sizeof(output));
        if (bits == 64) memcpy(root+8,&bias,sizeof(bias));
        else if (bits == 16) { uint16_t half = float_atomic_half(bias); memcpy(root+8,&half,sizeof(half)); }
        else { float narrow = (float)bias; memcpy(root+8,&narrow,sizeof(narrow)); }
        memcpy(map.data,root,sizeof(root)); void *values = (char *)map.data+1024;
        if (mode == 0) float_atomic_write(values,0,bits,1.5);
        if (mode == 2) { float_atomic_write(values,0,bits,1024); float_atomic_write(values,1,bits,-1024); }
        if (mode == 4) for (uint32_t i = 0; i < 128; ++i) float_atomic_write(values,i,bits,-128+i*0.5);
        TRY(ogpu_next_memory_flush(all)); signal.point.value = replay+1; pending = 1;
        TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),&submit));
        TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0; TRY(ogpu_next_memory_invalidate(all));
        if (mode == 0) REQUIRE(float_atomic_read(values,0,bits) == 1.5+(bits == 16 ? 80 : 2064)+128*bias);
        if (mode == 1) { REQUIRE(float_atomic_read(values,0,bits) == (bits == 16 ? 40 : 520)+64*bias); REQUIRE(float_atomic_read(values,1,bits) == (bits == 16 ? 40 : 1544)+64*bias); }
        if (mode == 2) { REQUIRE(float_atomic_read(values,0,bits) == -16+bias); REQUIRE(float_atomic_read(values,1,bits) == 15.75+bias); }
        if (mode == 3) for (uint32_t group = 0; group < 2; ++group) {
            REQUIRE(float_atomic_read(values,group*2,bits) == -16.0+group*16+bias); REQUIRE(float_atomic_read(values,group*2+1,bits) == -0.25+group*16+bias);
        }
        if (mode == 4) for (uint32_t i = 0; i < 128; ++i) {
            REQUIRE(float_atomic_read(values,i,bits) == i*0.25+bias); REQUIRE(float_atomic_read(values,i+128,bits) == -128+i*0.5);
        }
        if (mode == 5) for (uint32_t group = 0; group < 2; ++group) {
            uint32_t first = 0;
            for (uint32_t lane = 0; lane < 64; ++lane) {
                double previous = float_atomic_read(values,group*64+lane,bits);
                if (previous == -1024) ++first; else REQUIRE(previous == bias);
            }
            REQUIRE(first == 1); REQUIRE(float_atomic_read(values,128+group,bits) == bias);
        }
        REQUIRE(memcmp(map.data,&root,sizeof(root)) == 0);
        for (uint64_t i = sizeof(root); i < md.size; ++i)
            if (!(i >= 1024 && i < 1024+counts[mode]*width)) REQUIRE(((unsigned char *)map.data)[i] == 0xa5);
    }
    printf("Float%u atomic %s passes: independent profile, two workgroups, changed-input replay and guards.\n",width*8,entries[mode]);
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending float atomic work after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_executable_destroy(executable);
    ogpu_next_timeline_destroy(done); ogpu_next_memory_destroy(memory);
    if (file) fclose(file);
    free(code); free(scratch); return result;
}
