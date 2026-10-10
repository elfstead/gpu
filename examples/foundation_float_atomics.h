/* Native FP32 operations, independent memory domains. Exact binary fractions make
 * the finite-data oracle order-independent without promising arbitrary FP order. */
#define FLOAT_ATOMIC_FEATURES (OGPU_NEXT_FEATURE_BUFFER_FLOAT32_ATOMICS | OGPU_NEXT_FEATURE_BUFFER_FLOAT32_ATOMIC_ADD | OGPU_NEXT_FEATURE_BUFFER_FLOAT32_ATOMIC_MIN_MAX | OGPU_NEXT_FEATURE_SHARED_FLOAT32_ATOMICS | OGPU_NEXT_FEATURE_SHARED_FLOAT32_ATOMIC_ADD | OGPU_NEXT_FEATURE_SHARED_FLOAT32_ATOMIC_MIN_MAX)
#define FLOAT64_ATOMIC_FEATURES (OGPU_NEXT_FEATURE_BUFFER_FLOAT64_ATOMICS | OGPU_NEXT_FEATURE_BUFFER_FLOAT64_ATOMIC_ADD | OGPU_NEXT_FEATURE_BUFFER_FLOAT64_ATOMIC_MIN_MAX | OGPU_NEXT_FEATURE_SHARED_FLOAT64_ATOMICS | OGPU_NEXT_FEATURE_SHARED_FLOAT64_ATOMIC_ADD | OGPU_NEXT_FEATURE_SHARED_FLOAT64_ATOMIC_MIN_MAX)
static double float_atomic_read(const void *values, uint32_t index, uint32_t wide) {
    return wide ? ((const double *)values)[index] : ((const float *)values)[index];
}
static void float_atomic_write(void *values, uint32_t index, uint32_t wide, double value) {
    if (wide) ((double *)values)[index] = value; else ((float *)values)[index] = (float)value;
}
static int float_atomic_execution(ogpu_next_device *device, ogpu_next_memory_desc md, uint32_t domain,
                                  const char *path, uint32_t mode, uint32_t wide) {
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
    REQUIRE(mode < 6);
    uint32_t width = wide ? 8 : 4;
    uint64_t needed = wide ? (required[mode] << 6) | OGPU_NEXT_FEATURE_FLOAT64 : required[mode];
    ogpu_next_feature_info features = {0}; ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));
    TRY(ogpu_next_device_query(device,&q));
    if ((features.enabled & needed) != needed) { printf("Float%u atomic %s NOT exercised: feature unavailable.\n",width*8,entries[mode]); return EXIT_SUCCESS; }
    file = fopen(path,"rb"); REQUIRE(file != NULL);
    REQUIRE(fseek(file,0,SEEK_END) == 0); long size = ftell(file); REQUIRE(size >= 20 && size % 4 == 0);
    REQUIRE(fseek(file,0,SEEK_SET) == 0); code = malloc((size_t)size); REQUIRE(code != NULL);
    REQUIRE(fread(code,1,(size_t)size,file) == (size_t)size); fclose(file); file = NULL;
    ogpu_next_root_slot slot = {OGPU_NEXT_STAGE_COMPUTE,0,8};
    ogpu_next_argument_interface abi = {HEADER(ogpu_next_argument_interface,OGPU_NEXT_ARGUMENT_INTERFACE),8,1,&slot};
    ogpu_next_shader_requirements req = {HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),needed,{64,1,1},shared[mode]*(wide ? 2 : 1)};
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
        double bias = (replay ? -0.25 : 0.5) + (wide ? 0x1p-30 : 0);
        unsigned char root[16] = {0}; uint64_t output = address+1024;
        memcpy(root,&output,sizeof(output));
        if (wide) memcpy(root+8,&bias,sizeof(bias));
        else { float narrow = (float)bias; memcpy(root+8,&narrow,sizeof(narrow)); }
        memcpy(map.data,root,sizeof(root)); void *values = (char *)map.data+1024;
        if (mode == 0) float_atomic_write(values,0,wide,1.5);
        if (mode == 2) { float_atomic_write(values,0,wide,1024); float_atomic_write(values,1,wide,-1024); }
        if (mode == 4) for (uint32_t i = 0; i < 128; ++i) float_atomic_write(values,i,wide,-128+i*0.5);
        TRY(ogpu_next_memory_flush(all)); signal.point.value = replay+1; pending = 1;
        TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),&submit));
        TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0; TRY(ogpu_next_memory_invalidate(all));
        if (mode == 0) REQUIRE(float_atomic_read(values,0,wide) == 1.5+2064+128*bias);
        if (mode == 1) { REQUIRE(float_atomic_read(values,0,wide) == 520+64*bias); REQUIRE(float_atomic_read(values,1,wide) == 1544+64*bias); }
        if (mode == 2) { REQUIRE(float_atomic_read(values,0,wide) == -16+bias); REQUIRE(float_atomic_read(values,1,wide) == 15.75+bias); }
        if (mode == 3) for (uint32_t group = 0; group < 2; ++group) {
            REQUIRE(float_atomic_read(values,group*2,wide) == -16.0+group*16+bias); REQUIRE(float_atomic_read(values,group*2+1,wide) == -0.25+group*16+bias);
        }
        if (mode == 4) for (uint32_t i = 0; i < 128; ++i) {
            REQUIRE(float_atomic_read(values,i,wide) == i*0.25+bias); REQUIRE(float_atomic_read(values,i+128,wide) == -128+i*0.5);
        }
        if (mode == 5) for (uint32_t group = 0; group < 2; ++group) {
            uint32_t first = 0;
            for (uint32_t lane = 0; lane < 64; ++lane) {
                double previous = float_atomic_read(values,group*64+lane,wide);
                if (previous == -1024) ++first; else REQUIRE(previous == bias);
            }
            REQUIRE(first == 1); REQUIRE(float_atomic_read(values,128+group,wide) == bias);
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
