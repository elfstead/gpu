#define NUMERIC_FEATURES (OGPU_NEXT_FEATURE_FLOAT16 | OGPU_NEXT_FEATURE_INT8 | OGPU_NEXT_FEATURE_INT16 | OGPU_NEXT_FEATURE_INT64 | OGPU_NEXT_FEATURE_FLOAT64 | OGPU_NEXT_FEATURE_STORAGE8)
#define ATOMIC_FEATURES (OGPU_NEXT_FEATURE_INT64 | OGPU_NEXT_FEATURE_BUFFER_ATOMIC64 | OGPU_NEXT_FEATURE_SHARED_ATOMIC64)
#define SUBGROUP_FEATURES (OGPU_NEXT_FEATURE_SUBGROUP_SIZE_CONTROL | OGPU_NEXT_FEATURE_FULL_SUBGROUPS | OGPU_NEXT_FEATURE_SUBGROUP_EXTENDED_TYPES)
/* Scalar-width proof, not a tensor runtime or a numerical-throughput benchmark. */
static int numeric_execution(ogpu_next_device *device, ogpu_next_memory_desc desc, uint32_t domain, const char *path, uint32_t mode, uint32_t subgroup_size, uint32_t subgroup_flags) {
    uint32_t atomics = mode == 1, subgroups = mode == 2;
    int result = EXIT_FAILURE, pending = 0;
    FILE *file = NULL;
    void *code = NULL, *scratch = NULL;
    ogpu_next_executable *executable = NULL;
    ogpu_next_memory *memory = NULL;
    ogpu_next_arena *arena = NULL;
    ogpu_next_timeline *done = NULL;
    ogpu_next_feature_info features = {0};
    ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));
    TRY(ogpu_next_device_query(device,&q));
    ogpu_next_subgroup_limits subgroup_limits = {0};
    if (subgroups) {
        q = query(OGPU_NEXT_QUERY_SUBGROUP_LIMITS,&subgroup_limits,1,sizeof(subgroup_limits));
        TRY(ogpu_next_device_query(device,&q));
    }
    uint64_t required = subgroups ? ((subgroup_size || (subgroup_flags & OGPU_NEXT_SUBGROUP_ALLOW_VARYING) ? OGPU_NEXT_FEATURE_SUBGROUP_SIZE_CONTROL : 0)
        | (subgroup_flags & OGPU_NEXT_SUBGROUP_REQUIRE_FULL ? OGPU_NEXT_FEATURE_FULL_SUBGROUPS : 0)) : atomics ? ATOMIC_FEATURES : NUMERIC_FEATURES;
    if ((features.enabled & required) != required) {
        printf("%s fixture NOT exercised: missing feature mask 0x%llx.\n",atomics ? "Atomic" : "Combined numerical",(unsigned long long)(required & ~features.enabled));
        return EXIT_SUCCESS;
    }
    file = fopen(path,"rb"); REQUIRE(file != NULL);
    REQUIRE(fseek(file,0,SEEK_END) == 0);
    long size = ftell(file); REQUIRE(size >= 20 && size % 4 == 0);
    REQUIRE(fseek(file,0,SEEK_SET) == 0);
    code = malloc((size_t)size); REQUIRE(code != NULL);
    REQUIRE(fread(code,1,(size_t)size,file) == (size_t)size);
    fclose(file); file = NULL;
    ogpu_next_root_slot slot = {OGPU_NEXT_STAGE_COMPUTE,0,8};
    ogpu_next_argument_interface abi = {HEADER(ogpu_next_argument_interface,OGPU_NEXT_ARGUMENT_INTERFACE),8,1,&slot};
    ogpu_next_shader_requirements requirements = {HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),required,{64,1,1},atomics ? 8u : 0u};
    ogpu_next_subgroup_state subgroup = {HEADER(ogpu_next_subgroup_state,OGPU_NEXT_SUBGROUP_STATE),
        OGPU_NEXT_SUBGROUP_BASIC | OGPU_NEXT_SUBGROUP_ARITHMETIC | OGPU_NEXT_SUBGROUP_BALLOT,subgroup_size,subgroup_flags,0};
    ogpu_next_shader shader = {OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_SHADER_SPIRV,{code,(size_t)size},subgroups ? "subgroupMain" : atomics ? "atomicMain" : "numericMain",&abi.header,NULL,subgroups ? &subgroup.header : NULL};
    ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_COMPUTE,1,&shader,NULL,0,&requirements.header,NULL};
    if (subgroups) {
        subgroup.required_size = 3;
        REQUIRE(ogpu_next_executable_create(device,&ed,&executable) == OGPU_NEXT_INVALID && executable == NULL);
        subgroup.required_size = subgroup_size;
        subgroup.operations |= 256;
        REQUIRE(ogpu_next_executable_create(device,&ed,&executable) == OGPU_NEXT_UNSUPPORTED && executable == NULL);
        subgroup.operations &= ~256u;
    }
    TRY(ogpu_next_executable_create(device,&ed,&executable));
    free(code); code = NULL;
    TRY(ogpu_next_memory_create(device,&desc,&memory));
    ogpu_next_span all = {memory,0,desc.size};
    ogpu_next_mapping map = {0};
    TRY(ogpu_next_memory_map(all,&map));
    uint64_t address = 0; TRY(ogpu_next_memory_address(all,&address));
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1};
    TRY(ogpu_next_arena_create(device,&ad,&arena));
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    ogpu_next_encoder *encoder = NULL; ogpu_next_list *list = NULL;
    TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    ogpu_next_bind_executable(encoder,executable);
    ogpu_next_set_root(encoder,OGPU_NEXT_STAGE_COMPUTE,0,address);
    ogpu_next_launch launch = {{atomics ? 2u : 1u,1,1},0,NULL};
    ogpu_next_dispatch(encoder,&launch);
    ogpu_next_dependency dependency = {0};
    dependency.header = (ogpu_next_record)HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY);
    dependency.before = OGPU_NEXT_STAGE_COMPUTE; dependency.after = OGPU_NEXT_STAGE_HOST;
    dependency.global_before = OGPU_NEXT_ACCESS_SHADER_WRITE; dependency.global_after = OGPU_NEXT_ACCESS_HOST_READ;
    ogpu_next_barrier(encoder,&dependency);
    TRY(ogpu_next_commands_end(encoder,&list));
    TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_host_requirements sr = {0};
    TRY(ogpu_next_submit_scratch_requirements(1,0,1,&sr));
    REQUIRE(sr.alignment <= _Alignof(max_align_t));
    scratch = malloc((size_t)sr.size); REQUIRE(scratch != NULL);
    ogpu_next_sync_point signal = {{done,1},OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {HEADER(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),1,0,1,&list,NULL,&signal,scratch,sr.size};
    unsigned char *raw = map.data;
    uint8_t *bytes = raw+256;
    uint16_t *shorts = (uint16_t *)(raw+512), *halves = (uint16_t *)(raw+768);
    uint64_t *longs = (uint64_t *)(raw+1024);
    double *doubles = (double *)(raw+2048);
    for (uint32_t replay = 0; replay < 2; ++replay) {
        memset(map.data,0xa5,(size_t)desc.size);
        const uint64_t root[5] = {address+(atomics || subgroups ? 1024 : 256),subgroups ? replay : address+512,address+1024,address+2048,address+768};
        memcpy(map.data,root,sizeof(root));
        for (uint32_t i = 0; i < 64; ++i) {
            bytes[i] = (uint8_t)(200+5*i+replay);
            shorts[i] = (uint16_t)(65000+17*i+replay);
            longs[i] = UINT64_C(0xffffffff)+i+replay;
            doubles[i] = 16777216.0+i+replay*0.125;
            halves[i] = replay ? 0x4000 : 0x3c00;
        }
        if (atomics) { longs[0] = longs[1] = longs[2] = 0; longs[3] = replay; }
        TRY(ogpu_next_memory_flush(all));
        signal.point.value = replay+1;
        pending = 1; TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),&submit));
        TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0;
        TRY(ogpu_next_memory_invalidate(all));
        if (atomics) {
            REQUIRE(longs[0] == ((UINT64_C(8256)+128*replay)<<32)+384);
            REQUIRE(longs[1] == ((UINT64_C(2080)+64*replay)<<32)+192);
            REQUIRE(longs[2] == ((UINT64_C(6176)+64*replay)<<32)+192);
            REQUIRE(longs[3] == replay);
        }
        for (uint32_t i = 0; i < 64; ++i) {
            uint64_t x = UINT64_C(0xffffffff)+i+replay;
            if (subgroups) {
                const uint32_t *out = (const uint32_t *)(raw+1024)+i*8;
                uint32_t width = out[6], lane = out[7], sum = 0, prefix = 0, count = 0;
                REQUIRE(width > 0 && width <= 128 && lane < width);
                if (subgroup_size) REQUIRE(width == subgroup_size);
                else if (subgroup_flags & OGPU_NEXT_SUBGROUP_ALLOW_VARYING) REQUIRE(width >= subgroup_limits.min_size && width <= subgroup_limits.max_size);
                else REQUIRE(width == subgroup_limits.default_size);
                REQUIRE(out[lane/32] & (1u << (lane%32)));
                for (uint32_t bit = 0; bit < 128; ++bit) if (out[bit/32] & (1u << (bit%32))) {
                    REQUIRE(bit < width); ++count; sum += bit+replay;
                    if (bit < lane) prefix += bit+replay;
                }
                if (subgroup_flags & OGPU_NEXT_SUBGROUP_REQUIRE_FULL) REQUIRE(count == width);
                REQUIRE(out[4] == sum && out[5] == prefix);
                REQUIRE(bytes[i] == (uint8_t)(200+5*i+replay));
                REQUIRE(shorts[i] == (uint16_t)(65000+17*i+replay));
                REQUIRE(halves[i] == (replay ? 0x4000 : 0x3c00));
                continue;
            }
            if (atomics) {
                REQUIRE(bytes[i] == (uint8_t)(200+5*i+replay));
                REQUIRE(shorts[i] == (uint16_t)(65000+17*i+replay));
                if (i >= 4) REQUIRE(longs[i] == x);
                REQUIRE(doubles[i] == 16777216.0+i+replay*0.125);
                REQUIRE(halves[i] == (replay ? 0x4000 : 0x3c00));
                continue;
            }
            REQUIRE(bytes[i] == (uint8_t)(207+5*i+replay));
            REQUIRE(shorts[i] == (uint16_t)((65000+17*i+replay)*3+7));
            REQUIRE(longs[i] == ((x<<17) ^ (x>>7) ^ UINT64_C(0x123456789abcdef0)));
            REQUIRE(doubles[i] == (16777216.0+i+replay*0.125)*0.5+0.0625);
            REQUIRE(halves[i] == (replay ? 0x4480 : 0x4100));
        }
        REQUIRE(memcmp(map.data,root,sizeof(root)) == 0);
        for (size_t i = sizeof(root); i < desc.size; ++i)
            if (!((i >= 256 && i < 320) || (i >= 512 && i < 640) || (i >= 768 && i < 896)
                || (i >= 1024 && i < (subgroups ? 3072u : 1536u)) || (i >= 2048 && i < 2560))) REQUIRE(raw[i] == 0xa5);
    }
    if (subgroups) printf("Subgroups pass: requested size %u flags %u, ballot-checked reduction/prefix for every invocation, changed-seed replay and guards.\n",subgroup_size,subgroup_flags);
    else if (atomics) printf("Atomics pass: 64-bit address-buffer accumulation across workgroups, workgroup-shared reduction, explicit shader barriers, changed-seed replay and guards.\n");
    else printf("Numerics pass: native typed 8/16/64-bit integer storage, wraparound/high-word arithmetic, FP16/FP64 arithmetic, address roots, changed-input replay and guards.\n");
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending numerical work after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_executable_destroy(executable);
    ogpu_next_timeline_destroy(done); ogpu_next_memory_destroy(memory);
    if (file) fclose(file);
    free(code); free(scratch);
    return result;
}
