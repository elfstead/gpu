/* Physical tuples are discovery, not proof of numerical execution. */
static int matrix_discovery(ogpu_next_device *device) {
    int result = EXIT_FAILURE; ogpu_next_matrix_tuple *tuples = NULL;
    _Static_assert(sizeof(ogpu_next_matrix_tuple) == 48, "matrix tuple ABI");
    _Static_assert(offsetof(ogpu_next_matrix_tuple,stages) == 40, "matrix stages ABI");
    ogpu_next_feature_info features = {0}; ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));
    TRY(ogpu_next_device_query(device,&q));
    q = query(OGPU_NEXT_QUERY_MATRIX_TUPLES,NULL,0,sizeof(*tuples)); TRY(ogpu_next_device_query(device,&q));
    uint32_t count = q.count;
    if (count) {
        REQUIRE(features.available & OGPU_NEXT_FEATURE_COOPERATIVE_MATRIX);
        tuples = calloc(count,sizeof(*tuples)); REQUIRE(tuples != NULL); q.data = tuples; q.capacity = count;
        TRY(ogpu_next_device_query(device,&q)); REQUIRE(q.count == count);
        for (uint32_t i = 0; i < count; ++i) {
            REQUIRE(tuples[i].m && tuples[i].n && tuples[i].k);
            REQUIRE(tuples[i].a <= OGPU_NEXT_COMPONENT_UINT64 && tuples[i].b <= OGPU_NEXT_COMPONENT_UINT64
                && tuples[i].c <= OGPU_NEXT_COMPONENT_UINT64 && tuples[i].result <= OGPU_NEXT_COMPONENT_UINT64);
            REQUIRE(tuples[i].saturating <= 1 && tuples[i].reserved == 0);
            REQUIRE(tuples[i].scope == OGPU_NEXT_MATRIX_SCOPE_SUBGROUP && tuples[i].stages == OGPU_NEXT_STAGE_COMPUTE);
            printf("Matrix tuple %u: %ux%ux%u types=%u/%u/%u/%u saturation=%u\n",i,tuples[i].m,tuples[i].n,tuples[i].k,
                tuples[i].a,tuples[i].b,tuples[i].c,tuples[i].result,tuples[i].saturating);
        }
    }
    printf("Cooperative matrix discovery: %u tuples, feature available=%u enabled=%u.\n",
        count,!!(features.available & OGPU_NEXT_FEATURE_COOPERATIVE_MATRIX),!!(features.enabled & OGPU_NEXT_FEATURE_COOPERATIVE_MATRIX));
    result = EXIT_SUCCESS;
cleanup:
    free(tuples); return result;
}

#define MATRIX_FEATURES (OGPU_NEXT_FEATURE_COOPERATIVE_MATRIX | OGPU_NEXT_FEATURE_VULKAN_MEMORY_MODEL)
static int matrix_execution(ogpu_next_device *device, ogpu_next_memory_desc md, uint32_t domain, const char *path) {
    int result = EXIT_FAILURE, pending = 0; FILE *file = NULL; void *code = NULL, *scratch = NULL;
    ogpu_next_matrix_tuple *tuples = NULL; ogpu_next_executable *pipeline = NULL;
    ogpu_next_memory *memory = NULL; ogpu_next_arena *arena = NULL; ogpu_next_timeline *done = NULL;
    ogpu_next_feature_info features = {0}; ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));
    TRY(ogpu_next_device_query(device,&q));
    uint64_t required = MATRIX_FEATURES | OGPU_NEXT_FEATURE_FLOAT16 | OGPU_NEXT_FEATURE_SUBGROUP_SIZE_CONTROL | OGPU_NEXT_FEATURE_FULL_SUBGROUPS;
    if ((features.enabled & required) != required) { printf("Matrix execution NOT exercised: fixture features unavailable.\n"); return EXIT_SUCCESS; }
    q = query(OGPU_NEXT_QUERY_MATRIX_TUPLES,NULL,0,sizeof(*tuples)); TRY(ogpu_next_device_query(device,&q));
    uint32_t count = q.count, compatible = 0;
    if (count) { tuples = calloc(count,sizeof(*tuples)); REQUIRE(tuples != NULL); q.capacity = count; q.data = tuples;
        TRY(ogpu_next_device_query(device,&q)); }
    for (uint32_t i = 0; i < count; ++i) if (tuples[i].m == 8 && tuples[i].n == 8 && tuples[i].k == 8
        && tuples[i].a == OGPU_NEXT_COMPONENT_FLOAT16 && tuples[i].b == OGPU_NEXT_COMPONENT_FLOAT16
        && tuples[i].c == OGPU_NEXT_COMPONENT_FLOAT32 && tuples[i].result == OGPU_NEXT_COMPONENT_FLOAT32
        && !tuples[i].saturating && tuples[i].scope == OGPU_NEXT_MATRIX_SCOPE_SUBGROUP
        && (tuples[i].stages & OGPU_NEXT_STAGE_COMPUTE)) compatible = 1;
    free(tuples); tuples = NULL;
    if (!compatible) { printf("Matrix execution NOT exercised: no exact 8x8x8 FP16/FP32 tuple.\n"); return EXIT_SUCCESS; }
    ogpu_next_subgroup_limits limits = {0}; q = query(OGPU_NEXT_QUERY_SUBGROUP_LIMITS,&limits,1,sizeof(limits));
    TRY(ogpu_next_device_query(device,&q));
    if (!(limits.required_size_stages & OGPU_NEXT_STAGE_COMPUTE)) { printf("Matrix execution NOT exercised: exact compute subgroup size unavailable.\n"); return EXIT_SUCCESS; }
    uint32_t width = limits.default_size;
    file = fopen(path,"rb"); REQUIRE(file != NULL); REQUIRE(fseek(file,0,SEEK_END) == 0);
    long size = ftell(file); REQUIRE(size >= 20 && size % 4 == 0); REQUIRE(fseek(file,0,SEEK_SET) == 0);
    code = malloc((size_t)size); REQUIRE(code != NULL); REQUIRE(fread(code,1,(size_t)size,file) == (size_t)size); fclose(file); file = NULL;
    ogpu_next_root_slot slot = {OGPU_NEXT_STAGE_COMPUTE,0,8};
    ogpu_next_argument_interface abi = {HEADER(ogpu_next_argument_interface,OGPU_NEXT_ARGUMENT_INTERFACE),8,1,&slot};
    ogpu_next_specialization_entry entry = {0,0,4};
    ogpu_next_specialization spec = {HEADER(ogpu_next_specialization,OGPU_NEXT_SPECIALIZATION),1,0,&entry,{&width,4}};
    ogpu_next_subgroup_state subgroup = {HEADER(ogpu_next_subgroup_state,OGPU_NEXT_SUBGROUP_STATE),0,width,OGPU_NEXT_SUBGROUP_REQUIRE_FULL,0};
    ogpu_next_shader shader = {OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_SHADER_SPIRV,{code,(size_t)size},"matrixMain",&abi.header,&spec.header,&subgroup.header};
    ogpu_next_shader_requirements req = {HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),required,{width,1,1},0};
    ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_COMPUTE,1,&shader,NULL,0,&req.header,NULL,0,0};
    TRY(ogpu_next_executable_create(device,&ed,&pipeline)); free(code); code = NULL;
    TRY(ogpu_next_memory_create(device,&md,&memory)); ogpu_next_span all = {memory,0,md.size}; ogpu_next_mapping map = {0};
    TRY(ogpu_next_memory_map(all,&map)); uint64_t address = 0; TRY(ogpu_next_memory_address(all,&address));
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena));
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    ogpu_next_encoder *encoder = NULL; ogpu_next_list *list = NULL; TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
    ogpu_next_bind_executable(encoder,pipeline); ogpu_next_set_root(encoder,OGPU_NEXT_STAGE_COMPUTE,0,address);
    ogpu_next_launch launch = {{2,1,1},0,NULL}; ogpu_next_dispatch(encoder,&launch);
    ogpu_next_dependency dep = {HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_STAGE_HOST,
        OGPU_NEXT_ACCESS_SHADER_WRITE,OGPU_NEXT_ACCESS_HOST_READ,0,0,0,NULL,NULL,NULL,0}; ogpu_next_barrier(encoder,&dep);
    TRY(ogpu_next_commands_end(encoder,&list)); TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_host_requirements sr = {0}; TRY(ogpu_next_submit_scratch_requirements(1,0,1,&sr));
    REQUIRE(sr.alignment <= _Alignof(max_align_t)); scratch = malloc((size_t)sr.size); REQUIRE(scratch != NULL);
    ogpu_next_sync_point signal = {{done,1},OGPU_NEXT_STAGE_ALL};
    ogpu_next_submit_desc submit = {HEADER(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),1,0,1,&list,NULL,&signal,scratch,sr.size};
    for (uint32_t replay = 0; replay < 2; ++replay) {
        memset(map.data,0xa5,(size_t)md.size); uint64_t root[] = {address+256,address+512,address+768,address+1024};
        memcpy(map.data,root,sizeof(root));
        const uint16_t halves[] = {0,0x3c00,0x4000,0x4200,0x4400};
        uint16_t a[64], b[64]; float c[64];
        for (uint32_t r = 0; r < 8; ++r) for (uint32_t col = 0; col < 8; ++col) {
            a[r*8+col] = halves[r == col ? 2+replay : 1];
            b[col*8+r] = halves[r == 0 ? ((col+replay)%2 ? 4 : 2) : 1];
            c[r*8+col] = (r*8+col)*0.25f+replay*0.125f;
        }
        memcpy((char *)map.data+256,a,sizeof(a)); memcpy((char *)map.data+512,b,sizeof(b)); memcpy((char *)map.data+768,c,sizeof(c));
        TRY(ogpu_next_memory_flush(all)); signal.point.value = replay+1; pending = 1;
        TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),&submit));
        TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0; TRY(ogpu_next_memory_invalidate(all));
        const float *out = (const float *)((const char *)map.data+1024);
        for (uint32_t g = 0; g < 2; ++g) for (uint32_t r = 0; r < 8; ++r) for (uint32_t col = 0; col < 8; ++col) {
            float expected = c[r*8+col];
            for (uint32_t k = 0; k < 8; ++k) expected += (r == k ? 2+replay : 1)*(k == 0 ? ((col+replay)%2 ? 4 : 2) : 1);
            REQUIRE(out[g*64+r*8+col] == expected);
        }
        REQUIRE(memcmp(map.data,root,sizeof(root)) == 0); REQUIRE(memcmp((char *)map.data+256,a,sizeof(a)) == 0);
        REQUIRE(memcmp((char *)map.data+512,b,sizeof(b)) == 0); REQUIRE(memcmp((char *)map.data+768,c,sizeof(c)) == 0);
        for (uint64_t i = sizeof(root); i < md.size; ++i) if (!((i >= 256 && i < 384) || (i >= 512 && i < 640)
            || (i >= 768 && i < 1536))) REQUIRE(((unsigned char *)map.data)[i] == 0xa5);
    }
    printf("Matrix execution passes: exact 8x8x8 FP16/FP32 tuple, row/column-major inputs, two workgroups, changed-input replay and guards.\n");
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending matrix work after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_executable_destroy(pipeline); ogpu_next_timeline_destroy(done); ogpu_next_memory_destroy(memory);
    if (file) fclose(file);
    free(tuples); free(code); free(scratch); return result;
}
