/* GPU-produced executable choice, pointer/inline bytes, identity, launch and count.
 * No host readback or command expansion between producer and generated work. */
static int generated_execution(ogpu_next_device *device, ogpu_next_memory_desc md, uint32_t domain, char **paths) {
    int result = EXIT_FAILURE, pending = 0;
    FILE *file = NULL; void *code = NULL, *ss = NULL;
    ogpu_next_executable *pipelines[3] = {NULL};
    ogpu_next_generated_layout *layout = NULL; ogpu_next_generated_set *set = NULL;
    ogpu_next_memory *memory = NULL, *preprocess = NULL;
    ogpu_next_arena *arena = NULL; ogpu_next_timeline *done = NULL;
    ogpu_next_feature_info features = {0}; ogpu_next_generated_limits limits = {0};
    ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features)); TRY(ogpu_next_device_query(device,&q));
    q = query(OGPU_NEXT_QUERY_GENERATED_LIMITS,&limits,1,sizeof(limits)); TRY(ogpu_next_device_query(device,&q));
    if (!(features.enabled & OGPU_NEXT_FEATURE_GENERATED_COMMANDS) || !(limits.stages & OGPU_NEXT_STAGE_COMPUTE)
        || !(limits.pipeline_binding_stages & OGPU_NEXT_STAGE_COMPUTE)) {
        printf("GENERATED_UNVERIFIED native compute/executable selection unavailable; no fallback.\n"); return EXIT_SUCCESS;
    }
    ogpu_next_argument_interface abi = {HEADER(ogpu_next_argument_interface,OGPU_NEXT_ARGUMENT_INTERFACE),32,0,NULL};
    ogpu_next_shader_requirements requirements = {HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),0,{1,1,1},0};
    uint32_t extra = 0;
    ogpu_next_specialization_entry entry = {9,0,4};
    ogpu_next_specialization spec = {HEADER(ogpu_next_specialization,OGPU_NEXT_SPECIALIZATION),1,0,&entry,{&extra,4}};
    for (uint32_t i = 0; i < 3; ++i) {
        file = fopen(paths[i != 0],"rb"); REQUIRE(file);
        REQUIRE(fseek(file,0,SEEK_END) == 0); long size = ftell(file); REQUIRE(size >= 20 && size % 4 == 0);
        REQUIRE(fseek(file,0,SEEK_SET) == 0); code = malloc((size_t)size); REQUIRE(code);
        REQUIRE(fread(code,1,(size_t)size,file) == (size_t)size); fclose(file); file = NULL;
        extra = i*100;
        ogpu_next_shader shader = {OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_SHADER_SPIRV,{code,(size_t)size},i ? "consume" : "produce",&abi.header,i ? &spec.header : NULL,NULL};
        ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_COMPUTE,
            1,&shader,NULL,0,&requirements.header,NULL,i ? OGPU_NEXT_COMPILE_INDIRECT_BINDABLE : 0,0};
        TRY(ogpu_next_executable_create(device,&ed,&pipelines[i])); free(code); code = NULL;
    }
    ogpu_next_generated_set_desc sd = {HEADER(ogpu_next_generated_set_desc,OGPU_NEXT_GENERATED_SET_DESC),pipelines[0],2,0};
    REQUIRE(ogpu_next_generated_set_create(device,&sd,&set) == OGPU_NEXT_INVALID && !set);
    sd.initial = pipelines[1]; TRY(ogpu_next_generated_set_create(device,&sd,&set));
    md.size = 4096; TRY(ogpu_next_memory_create(device,&md,&memory));
    ogpu_next_span all = {memory,0,md.size}; ogpu_next_mapping map = {0}; TRY(ogpu_next_memory_map(all,&map));
    uint64_t base = 0; TRY(ogpu_next_memory_address(all,&base));
    ogpu_next_host_requirements sr = {0}; TRY(ogpu_next_submit_scratch_requirements(1,0,1,&sr));
    REQUIRE(sr.alignment <= _Alignof(max_align_t)); ss = malloc((size_t)sr.size); REQUIRE(ss);
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena)); TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_ONE_SHOT,OGPU_NEXT_PRIMARY,NULL};
    uint64_t value = 0;
    for (uint32_t selection = 0; selection < 2; ++selection) for (uint32_t separate = 0; separate < 2; ++separate) {
        ogpu_next_generated_token tokens[] = {
            {OGPU_NEXT_GENERATED_EXECUTABLE,0,0,0}, {OGPU_NEXT_GENERATED_INLINE,8,0,24},
            {OGPU_NEXT_GENERATED_INLINE,36,28,4}, {OGPU_NEXT_GENERATED_SEQUENCE_INDEX,40,24,4},
            {OGPU_NEXT_GENERATED_DISPATCH,40,0,0}};
        ogpu_next_generated_layout_desc ld = {HEADER(ogpu_next_generated_layout_desc,OGPU_NEXT_GENERATED_LAYOUT_DESC),
            OGPU_NEXT_STAGE_COMPUTE,separate | (selection ? OGPU_NEXT_GENERATED_UNORDERED : 0),56,selection ? 5 : 4,0,tokens+(selection ? 0 : 1)};
        TRY(ogpu_next_generated_layout_create(device,&ld,&layout));
        for (uint32_t seed = 0; seed < 5; ++seed) {
            ogpu_next_generated_set_write write = {1,0,pipelines[seed == 4 ? 1 : 2]}; TRY(ogpu_next_generated_set_update(set,1,&write));
            ogpu_next_generated_set_write duplicates[2] = {{1,0,pipelines[1]},{1,0,pipelines[1]}};
            REQUIRE(ogpu_next_generated_set_update(set,2,duplicates) == OGPU_NEXT_INVALID);
            ogpu_next_generated_set_write outside = {2,0,pipelines[1]};
            REQUIRE(ogpu_next_generated_set_update(set,1,&outside) == OGPU_NEXT_INVALID);
            ogpu_next_generated_work work = {layout,selection ? set : NULL,selection ? NULL : pipelines[1],2,0};
            ogpu_next_generated_requirements_info gr = {0}; TRY(ogpu_next_generated_requirements(device,&work,&gr));
            ogpu_next_span storage = {NULL,0,0};
            if (gr.size) {
                ogpu_next_memory_desc pd = {HEADER(ogpu_next_memory_desc,OGPU_NEXT_MEMORY_DESC),gr.size,gr.alignment,
                    OGPU_NEXT_USAGE_PREPROCESS,0,OGPU_NEXT_MEMORY_LINEAR,NULL,0,0};
                uint32_t types[32]; ogpu_next_requirements req = {0}; req.compatible_types = types; req.compatible_type_capacity = 32;
                TRY(ogpu_next_memory_requirements(device,&pd,&req)); pd.memory_type = UINT32_MAX;
                for (uint32_t t = 0; t < req.compatible_type_count; ++t) if (gr.memory_type_bits & (UINT32_C(1) << types[t])) { pd.memory_type = types[t]; break; }
                REQUIRE(pd.memory_type != UINT32_MAX); TRY(ogpu_next_memory_create(device,&pd,&preprocess));
                storage = (ogpu_next_span){preprocess,0,gr.size};
            }
            memset(map.data,0xa5,(size_t)md.size); TRY(ogpu_next_memory_flush(all));
            struct { uint64_t output, records, count; uint32_t sequence, seed; } control = {base+1024,base+64,base+512,0,seed};
            _Static_assert(sizeof(control) == 32, "generated shader control layout");
            ogpu_next_encoder *encoder = NULL; ogpu_next_list *list = NULL; TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
            ogpu_next_bind_executable(encoder,pipelines[0]); ogpu_next_set_inline(encoder,OGPU_NEXT_STAGE_COMPUTE,0,sizeof(control),&control);
            ogpu_next_launch launch = {{1,1,1},0,NULL}; ogpu_next_dispatch(encoder,&launch);
            ogpu_next_dependency dep = {HEADER(ogpu_next_dependency,OGPU_NEXT_DEPENDENCY),OGPU_NEXT_STAGE_COMPUTE,
                separate ? OGPU_NEXT_STAGE_PREPROCESS : OGPU_NEXT_STAGE_INDIRECT,OGPU_NEXT_ACCESS_STORAGE_WRITE,
                separate ? OGPU_NEXT_ACCESS_PREPROCESS_READ : OGPU_NEXT_ACCESS_INDIRECT_READ,0,0,0,NULL,NULL,NULL,0};
            ogpu_next_barrier(encoder,&dep); ogpu_next_bind_executable(encoder,pipelines[1]);
            uint64_t zero[4] = {0}; ogpu_next_set_inline(encoder,OGPU_NEXT_STAGE_COMPUTE,0,sizeof(zero),zero);
            ogpu_next_generated_batch batch = {work,{memory,64,112},storage,seed == 3 ? (ogpu_next_span){NULL,0,0} : (ogpu_next_span){memory,512,4}};
            if (separate) {
                ogpu_next_generated_preprocess(encoder,&batch,encoder);
                dep.before = OGPU_NEXT_STAGE_PREPROCESS; dep.after = OGPU_NEXT_STAGE_INDIRECT;
                dep.global_before = OGPU_NEXT_ACCESS_PREPROCESS_WRITE; dep.global_after = OGPU_NEXT_ACCESS_INDIRECT_READ;
                ogpu_next_barrier(encoder,&dep);
            }
            ogpu_next_generated_execute(encoder,&batch,separate);
            dep.before = OGPU_NEXT_STAGE_COMPUTE; dep.after = OGPU_NEXT_STAGE_HOST;
            dep.global_before = OGPU_NEXT_ACCESS_STORAGE_WRITE; dep.global_after = OGPU_NEXT_ACCESS_HOST_READ; ogpu_next_barrier(encoder,&dep);
            TRY(ogpu_next_commands_end(encoder,&list));
            ogpu_next_sync_point signal = {{done,++value},OGPU_NEXT_STAGE_ALL};
            ogpu_next_submit_desc submit = {HEADER(ogpu_next_submit_desc,OGPU_NEXT_SUBMIT_DESC),1,0,1,&list,NULL,&signal,ss,sr.size};
            pending = 1; TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),&submit));
            TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0; TRY(ogpu_next_memory_invalidate(all));
            uint32_t expected[1024]; for (uint32_t i = 0; i < 1024; ++i) expected[i] = UINT32_C(0xa5a5a5a5);
            for (uint32_t i = 0; i < 2; ++i) {
                uint32_t *r = expected+16+i*14; r[0] = (i+seed)&1;
                r[2] = (uint32_t)(base+1024); r[3] = (uint32_t)((base+1024)>>32);
                r[4] = r[5] = r[6] = r[7] = 0; r[9] = seed+i*7; r[10] = r[11] = r[12] = 1;
                if (i < (seed == 3 ? 2 : seed%3)) expected[256+i] = seed+i*7+(selection && seed != 4 && r[0] ? 200 : 100);
            }
            expected[128] = seed%3;
            if (memcmp(expected,map.data,sizeof(expected)) != 0) {
                for (uint32_t i = 0; i < 1024; ++i) if (expected[i] != ((uint32_t *)map.data)[i]) {
                    fprintf(stderr,"Generated selection=%u separate=%u seed=%u word=%u got=%08x expected=%08x\n",selection,separate,seed,i,((uint32_t *)map.data)[i],expected[i]); break;
                }
                REQUIRE(0);
            }
            TRY(ogpu_next_arena_reset(arena));
            if (seed == 4) for (uint32_t bad = 0; bad < 5; ++bad) {
                TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
                if (bad != 0) ogpu_next_bind_executable(encoder,pipelines[1]);
                ogpu_next_generated_batch invalid = batch;
                if (bad == 1) invalid.arguments.size = 1;
                if (bad == 2) invalid.count.size = 3;
                if (bad == 3) invalid.work.max_sequences = 0;
                ogpu_next_generated_execute(encoder,&invalid,bad == 4 ? separate^1 : separate);
                REQUIRE(ogpu_next_commands_end(encoder,&list) == OGPU_NEXT_INVALID && !list);
                TRY(ogpu_next_arena_reset(arena));
            }
            ogpu_next_memory_destroy(preprocess); preprocess = NULL;
        }
        ogpu_next_generated_layout_destroy(layout); layout = NULL;
    }
    printf("GENERATED_PASS submissions=%llu modes=4 gpu_choice=1 gpu_count=1 gpu_arguments=1 sequence_identity=1 set_update=1 guards=1 rejected_recordings=20\n",(unsigned long long)value);
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending generated work after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_generated_layout_destroy(layout); ogpu_next_generated_set_destroy(set);
    for (uint32_t i = 0; i < 3; ++i) ogpu_next_executable_destroy(pipelines[i]);
    ogpu_next_timeline_destroy(done); ogpu_next_memory_destroy(preprocess); ogpu_next_memory_destroy(memory);
    if (file) fclose(file);
    free(code); free(ss); return result;
}
