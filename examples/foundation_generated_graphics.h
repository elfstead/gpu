/* A bounded GPU-culled scene composed entirely through the installed API.
 * Producer, compaction policy, storage and expected image belong to the consumer. */
static int generated_graphics(ogpu_next_device *device, ogpu_next_memory_desc md, uint32_t domain, char **paths) {
    int result = EXIT_FAILURE, pending = 0;
    FILE *file = NULL; void *code[3] = {NULL}, *bs = NULL, *rs = NULL, *ss = NULL;
    ogpu_next_argument_layout *producer_arguments = NULL, *draw_arguments = NULL;
    ogpu_next_executable *producer = NULL, *pipelines[2] = {NULL};
    ogpu_next_generated_layout *layout = NULL; ogpu_next_generated_set *set = NULL;
    ogpu_next_memory *memory = NULL, *backing = NULL, *preprocess = NULL;
    ogpu_next_image *image = NULL; ogpu_next_view *view = NULL;
    ogpu_next_arena *arena = NULL; ogpu_next_timeline *done = NULL;
    ogpu_next_feature_info features = {0}; ogpu_next_generated_limits limits = {0};
    ogpu_next_query q = query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features)); TRY(ogpu_next_device_query(device,&q));
    q = query(OGPU_NEXT_QUERY_GENERATED_LIMITS,&limits,1,sizeof(limits)); TRY(ogpu_next_device_query(device,&q));
    const uint64_t stages = OGPU_NEXT_STAGE_VERTEX | OGPU_NEXT_STAGE_FRAGMENT;
    if (!(features.enabled & OGPU_NEXT_FEATURE_GENERATED_COMMANDS) || (limits.stages & stages) != stages) {
        printf("GENERATED_GRAPHICS_UNVERIFIED native graphics unavailable stages=%llu binding_stages=%llu; no fallback.\n",
            (unsigned long long)limits.stages,(unsigned long long)limits.pipeline_binding_stages); return EXIT_SUCCESS;
    }
    uint32_t selections = (limits.pipeline_binding_stages & stages) == stages ? 2 : 1;
    if (selections == 1) printf("GENERATED_GRAPHICS_SELECTION_UNVERIFIED native executable selection unavailable; fixed-executable cases remain.\n");
    ogpu_next_argument_field producer_field = {0,32,OGPU_NEXT_ARGUMENT_INLINE,0,OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_ARGUMENT_DIRECT,OGPU_NEXT_ARGUMENT_DIRECT};
    ogpu_next_argument_layout_desc argument_desc = {OGPU_NEXT_ARGUMENT_COMPUTE,32,1,0,&producer_field};
    TRY(ogpu_next_argument_layout_create(device,&argument_desc,&producer_arguments));
    ogpu_next_argument_field draw_fields[] = {
        {0,4,OGPU_NEXT_ARGUMENT_INLINE,0,stages,OGPU_NEXT_ARGUMENT_DIRECT,OGPU_NEXT_ARGUMENT_DIRECT},
        {4,4,OGPU_NEXT_ARGUMENT_INLINE,0,stages,OGPU_NEXT_ARGUMENT_DIRECT,OGPU_NEXT_ARGUMENT_DIRECT},
        {8,8,OGPU_NEXT_ARGUMENT_INLINE,0,stages,OGPU_NEXT_ARGUMENT_DIRECT,OGPU_NEXT_ARGUMENT_DIRECT}};
    argument_desc = (ogpu_next_argument_layout_desc){OGPU_NEXT_ARGUMENT_GRAPHICS,16,3,0,draw_fields};
    TRY(ogpu_next_argument_layout_create(device,&argument_desc,&draw_arguments));
    ogpu_next_shader shaders[3];
    const uint32_t shader_stages[] = {OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_STAGE_FRAGMENT};
    const char *entries[] = {"produce","vertexMain","fragmentMain"};
    for (uint32_t i = 0; i < 3; ++i) {
        file = fopen(paths[i],"rb"); REQUIRE(file);
        REQUIRE(fseek(file,0,SEEK_END) == 0); long size = ftell(file); REQUIRE(size >= 20 && size % 4 == 0);
        REQUIRE(fseek(file,0,SEEK_SET) == 0); code[i] = malloc((size_t)size); REQUIRE(code[i]);
        REQUIRE(fread(code[i],1,(size_t)size,file) == (size_t)size); fclose(file); file = NULL;
        shaders[i] = (ogpu_next_shader){shader_stages[i],OGPU_NEXT_SHADER_SPIRV,{code[i],(size_t)size},entries[i],NULL,NULL};
    }
    ogpu_next_shader_requirements shader_req = {HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),0,{1,1,1},0};
    ogpu_next_executable_desc ed = {HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_COMPUTE,
        1,shaders,NULL,0,&shader_req.header,NULL,0,0};
    TRY(ogpu_next_executable_create(device,&ed,producer_arguments,&producer));
    ogpu_next_color_state color = {OGPU_NEXT_RGBA8_UNORM,15,0,0,0,0,0,0,0};
    ogpu_next_graphics_state gs = {HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),
        OGPU_NEXT_TRIANGLES,OGPU_NEXT_CULL_NONE,OGPU_NEXT_FRONT_CCW,1,1,&color,0,0,0,0,{0},0,{0},{0},NULL,NULL,0,0};
    uint32_t variant = 0;
    ogpu_next_specialization_entry entry = {9,0,4};
    ogpu_next_specialization spec = {HEADER(ogpu_next_specialization,OGPU_NEXT_SPECIALIZATION),1,0,&entry,{&variant,4}};
    shaders[1].specialization = &spec.header;
    ogpu_next_shader_requirements graphics_req = {HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),OGPU_NEXT_FEATURE_RASTER,{0,0,0},0};
    ed = (ogpu_next_executable_desc){HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_GRAPHICS,
        2,shaders+1,&gs.header,OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR,&graphics_req.header,NULL,selections == 2 ? OGPU_NEXT_COMPILE_INDIRECT_BINDABLE : 0,0};
    for (variant = 0; variant < 2; ++variant) TRY(ogpu_next_executable_create(device,&ed,draw_arguments,&pipelines[variant]));
    for (uint32_t i = 0; i < 3; ++i) { free(code[i]); code[i] = NULL; }
    ogpu_next_generated_set_desc sd = {HEADER(ogpu_next_generated_set_desc,OGPU_NEXT_GENERATED_SET_DESC),pipelines[0],2,0};
    if (selections == 2) TRY(ogpu_next_generated_set_create(device,&sd,&set));
    md.size = 32768; TRY(ogpu_next_memory_create(device,&md,&memory));
    ogpu_next_span all = {memory,0,md.size}; ogpu_next_mapping map = {0}; TRY(ogpu_next_memory_map(all,&map));
    uint64_t base = 0; TRY(ogpu_next_memory_address(all,&base));
    ogpu_next_subresources range = {OGPU_NEXT_ASPECT_COLOR,0,1,0,1};
    ogpu_next_image_desc id = {HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),OGPU_NEXT_RGBA8_UNORM,
        OGPU_NEXT_IMAGE_2D,1,1,1,{64,32,1},OGPU_NEXT_IMAGE_COPY_SRC | OGPU_NEXT_IMAGE_COLOR_ATTACHMENT,0,NULL,0,0,NULL};
    uint32_t types[32]; ogpu_next_requirements requirements = {0};
    requirements.compatible_types = types; requirements.compatible_type_capacity = 32;
    TRY(ogpu_next_image_requirements(device,&id,&requirements)); REQUIRE(requirements.compatible_type_count);
    TRY(ogpu_next_image_create_unbound(device,&id,&image));
    TRY(ogpu_next_memory_create_dedicated_image(image,types[0],&backing));
    TRY(ogpu_next_image_bind(image,(ogpu_next_span){backing,0,requirements.size}));
    ogpu_next_view_desc vd = {HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),OGPU_NEXT_RGBA8_UNORM,
        OGPU_NEXT_VIEW_2D,OGPU_NEXT_IMAGE_COLOR_ATTACHMENT,{0},range}; TRY(ogpu_next_view_create(image,&vd,&view));
    ogpu_next_host_requirements br = {0}, rr = {0}, sr = {0};
    TRY(ogpu_next_barrier_scratch_requirements(device,1,0,1,&br)); TRY(ogpu_next_render_scratch_requirements(1,&rr));
    TRY(ogpu_next_submit_scratch_requirements(device,1,&(ogpu_next_submit_desc){.list_count=1,.signal_count=1},&sr));
    REQUIRE(br.alignment <= _Alignof(max_align_t) && rr.alignment <= _Alignof(max_align_t) && sr.alignment <= _Alignof(max_align_t));
    bs = malloc((size_t)br.size); rs = malloc((size_t)rr.size); ss = malloc((size_t)sr.size); REQUIRE(bs && rs && ss);
    ogpu_next_arena_desc ad = {HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};
    TRY(ogpu_next_arena_create(device,&ad,&arena)); TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_recording_desc rd = {HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_ONE_SHOT,OGPU_NEXT_PRIMARY,NULL};
    ogpu_next_attachment attachment = {view,NULL,OGPU_NEXT_STATE_COLOR_ATTACHMENT,0,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.f32={0,0,0,1}}};
    ogpu_next_render_desc render = {HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),0,0,64,32,1,0,1,0,1,&attachment,NULL,NULL,{rs,rr.size}};
    ogpu_next_viewport_state vp = {HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),0,0,64,32,0,1,0,0,64,32};
    ogpu_next_image_copy copy = {{OGPU_NEXT_ASPECT_COLOR,0,0,1,{0,0,0},{64,32,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};
    const uint32_t columns[] = {0,1,4,8,3,5,2};
    const uint32_t commands[] = {2,0,1,3,4,2,3}; /* 4 selects per-object 0/1/2. */
    const uint32_t maxima[] = {2,2,2,2,2,0,1};
    const uint32_t actions[] = {OGPU_NEXT_GENERATED_DRAW,OGPU_NEXT_GENERATED_DRAW_INDEXED,
        OGPU_NEXT_GENERATED_DRAW_COUNT,OGPU_NEXT_GENERATED_DRAW_INDEXED_COUNT};
    uint32_t profiles = 0;
    uint64_t value = 0;
    for (uint32_t action = 0; action < 4; ++action) {
        if (!(limits.tokens & (UINT64_C(1) << actions[action]))) {
            printf("GENERATED_GRAPHICS_ACTION_UNVERIFIED token=%u; no fallback.\n",actions[action]); continue;
        }
        uint32_t indexed = action & 1, nested = action >= 2;
        ++profiles;
        for (uint32_t selection = 0; selection < selections; ++selection) for (uint32_t separate = 0; separate < 2; ++separate) {
            ogpu_next_generated_token tokens[] = {
                {OGPU_NEXT_GENERATED_EXECUTABLE,0,0,0}, {OGPU_NEXT_GENERATED_ARGUMENT,4,0,4},
                {OGPU_NEXT_GENERATED_ARGUMENT,8,8,4}, {OGPU_NEXT_GENERATED_SEQUENCE_INDEX,12,4,4},
                {actions[action],16,0,0}};
            ogpu_next_generated_layout_desc ld = {HEADER(ogpu_next_generated_layout_desc,OGPU_NEXT_GENERATED_LAYOUT_DESC),
                draw_arguments,stages,separate,64,selection ? 5 : 4,0,tokens+(selection ? 0 : 1)};
            TRY(ogpu_next_generated_layout_create(device,&ld,&layout));
            ogpu_next_generated_work work = {layout,selection ? set : NULL,selection ? NULL : pipelines[0],32,nested ? 2 : 0};
            for (uint32_t run = 0; run < (nested ? 7u : 5u); ++run) {
                uint32_t n = columns[run];
                work.max_draws = nested ? maxima[run] : 0;
                ogpu_next_generated_set_write write = {1,0,pipelines[run == 4 ? 0 : 1]};
                if (set) TRY(ogpu_next_generated_set_update(set,1,&write));
                /* Cold requirements follow each set update and exact work choice. */
                ogpu_next_generated_requirements_info gr = {0}; TRY(ogpu_next_generated_requirements(device,&work,&gr));
                ogpu_next_span storage = {NULL,0,0};
                if (gr.size) {
                    ogpu_next_memory_desc pd = {HEADER(ogpu_next_memory_desc,OGPU_NEXT_MEMORY_DESC),gr.size,gr.alignment,
                        OGPU_NEXT_USAGE_PREPROCESS,0,OGPU_NEXT_MEMORY_LINEAR,NULL,0,0};
                    ogpu_next_requirements req = {0}; req.compatible_types = types; req.compatible_type_capacity = 32;
                    TRY(ogpu_next_memory_requirements(device,&pd,&req)); pd.memory_type = UINT32_MAX;
                    for (uint32_t t = 0; t < req.compatible_type_count; ++t) if (gr.memory_type_bits & (UINT32_C(1) << types[t])) { pd.memory_type = types[t]; break; }
                    REQUIRE(pd.memory_type != UINT32_MAX); TRY(ogpu_next_memory_create(device,&pd,&preprocess));
                    storage = (ogpu_next_span){preprocess,0,gr.size};
                }
                memset(map.data,0xa5,(size_t)md.size);
                /* First index skips a guard; signed base vertex maps 2..7 to 0..5. */
                for (uint32_t i = 0; i < 12; ++i) ((uint32_t *)map.data)[1793+i] = 2+i%6;
                TRY(ogpu_next_memory_flush(all));
                struct { uint64_t records, count; uint32_t columns, action, commands, pad; } control = {base,base+2048,n,action,commands[run],0};
                _Static_assert(sizeof(control) == 32,"generated graphics producer layout");
                ogpu_next_encoder *encoder = NULL; ogpu_next_list *list = NULL; TRY(ogpu_next_commands_begin(arena,&rd,&encoder));
                ogpu_next_bind_executable(encoder,producer); ogpu_next_set_arguments(encoder,producer_arguments,0,sizeof(control),&control);
                ogpu_next_launch launch = {{1,1,1},0,NULL}; ogpu_next_dispatch(encoder,&launch);
                ogpu_next_barrier_scope scope = {OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_STAGE_INDIRECT | (separate ? OGPU_NEXT_STAGE_PREPROCESS : 0),
                    OGPU_NEXT_ACCESS_STORAGE_WRITE,OGPU_NEXT_ACCESS_INDIRECT_READ | (separate ? OGPU_NEXT_ACCESS_PREPROCESS_READ : 0)};
                ogpu_next_host_span scratch = {bs,br.size};
                ogpu_next_dependency dep = {1,0,0,0,&scope,NULL,NULL};
                common_barrier(encoder,&dep,&scope,scratch);
                ogpu_next_bind_executable(encoder,pipelines[0]); ogpu_next_set_graphics_state(encoder,&vp.header);
                if (indexed) ogpu_next_bind_indices(encoder,(ogpu_next_span){memory,7168,52},OGPU_NEXT_INDEX_U32);
                uint32_t zero[4] = {0,0,0,nested}; ogpu_next_set_arguments(encoder,draw_arguments,0,sizeof(zero),zero);
                ogpu_next_generated_batch batch = {work,{memory,0,2048},storage,run == 3 ? (ogpu_next_span){NULL,0,0} : (ogpu_next_span){memory,2048,4}};
                if (separate) {
                    ogpu_next_generated_preprocess(encoder,&batch,encoder);
                    scope = (ogpu_next_barrier_scope){OGPU_NEXT_STAGE_PREPROCESS,OGPU_NEXT_STAGE_INDIRECT,OGPU_NEXT_ACCESS_PREPROCESS_WRITE,OGPU_NEXT_ACCESS_INDIRECT_READ};
                    common_barrier(encoder,&dep,&scope,scratch);
                }
                ogpu_next_image_barrier barrier = {{0,0,0,OGPU_NEXT_ACCESS_COLOR_WRITE},image,range,OGPU_NEXT_STATE_UNDEFINED,
                    OGPU_NEXT_STATE_COLOR_ATTACHMENT,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1,0};
                scope = (ogpu_next_barrier_scope){0,OGPU_NEXT_STAGE_COLOR,0,0}; dep.image_count = 1; dep.images = &barrier;
                common_barrier(encoder,&dep,&scope,scratch);
                ogpu_next_render_begin(encoder,&render); ogpu_next_generated_execute(encoder,&batch,separate); ogpu_next_render_end(encoder);
                scope = (ogpu_next_barrier_scope){OGPU_NEXT_STAGE_COLOR,OGPU_NEXT_STAGE_COPY,0,0};
                barrier.scope.access_before = OGPU_NEXT_ACCESS_COLOR_WRITE; barrier.scope.access_after = OGPU_NEXT_ACCESS_COPY_READ;
                barrier.old_state = OGPU_NEXT_STATE_COLOR_ATTACHMENT; barrier.new_state = OGPU_NEXT_STATE_COPY_SRC; barrier.discard = 0;
                common_barrier(encoder,&dep,&scope,scratch);
                ogpu_next_copy_from_image(encoder,(ogpu_next_span){memory,8192,8192},image,&copy);
                scope = (ogpu_next_barrier_scope){OGPU_NEXT_STAGE_COPY | OGPU_NEXT_STAGE_COMPUTE,OGPU_NEXT_STAGE_HOST,
                    OGPU_NEXT_ACCESS_COPY_WRITE | OGPU_NEXT_ACCESS_STORAGE_WRITE,OGPU_NEXT_ACCESS_HOST_READ};
                dep.image_count = 0; dep.images = NULL; common_barrier(encoder,&dep,&scope,scratch);
                TRY(ogpu_next_commands_end(encoder,&list));
                ogpu_next_sync_point signal = {{done,++value},OGPU_NEXT_STAGE_ALL};
                ogpu_next_submit_desc submit = {1,0,1,0,0,0,&list,NULL,&signal,NULL,NULL};
                ogpu_next_submit_outcome outcome;
                pending = 1; TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),1,&submit,&outcome,(ogpu_next_host_span){ss,sr.size}));
                TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000))); pending = 0; TRY(ogpu_next_memory_invalidate(all));
                uint32_t expected[8192]; for (uint32_t i = 0; i < 8192; ++i) expected[i] = UINT32_C(0xa5a5a5a5);
                uint32_t sequence = 0;
                for (uint32_t object = 0; object < 32; ++object) if (object%8 < n) {
                    uint32_t *r = expected+sequence*16;
                    r[0] = (object+n)&1; r[1] = object; r[2] = n;
                    if (nested) {
                        uint64_t address = base+4096+sequence*64;
                        r[4] = (uint32_t)address; r[5] = (uint32_t)(address>>32); r[6] = 32;
                        r[7] = commands[run] == 4 ? object%3 : commands[run];
                        for (uint32_t j = 0; j < 2; ++j) {
                            uint32_t *d = expected+1024+sequence*16+j*8;
                            d[0] = 6; d[1] = 1; d[2] = indexed ? 1+j*6 : 0;
                            d[3] = indexed ? (uint32_t)-2 : j;
                            if (indexed) d[4] = j;
                        }
                    } else {
                        r[4] = 6; r[5] = 1; r[6] = indexed ? 1 : 0; r[7] = indexed ? (uint32_t)-2 : 0;
                        if (indexed) r[8] = 0;
                    }
                    ++sequence;
                }
                expected[512] = sequence;
                for (uint32_t i = 0; i < 12; ++i) expected[1793+i] = 2+i%6;
                for (uint32_t y = 0; y < 32; ++y) for (uint32_t x = 0; x < 64; ++x) {
                    uint32_t object = (y/8)*8+x/8, pixel = UINT32_C(0xff000000);
                    uint32_t count = commands[run] == 4 ? object%3 : commands[run];
                    if (count > work.max_draws) count = work.max_draws;
                    if (x/8 < n && (!nested || (x%8)/4 < count)) {
                        uint32_t identity = (y/8)*n+x/8;
                        pixel |= (object&1)*255 | (selection && run != 4 && ((object+n)&1) ? UINT32_C(0xff00) : 0)
                            | (((n+identity)&1)*255 << 16);
                    }
                    expected[2048+y*64+x] = pixel;
                }
                if (memcmp(expected,map.data,sizeof(expected))) {
                    for (uint32_t i = 0; i < 8192; ++i) if (expected[i] != ((uint32_t *)map.data)[i]) {
                        fprintf(stderr,"Generated graphics action=%u selection=%u separate=%u columns=%u word=%u got=%08x expected=%08x\n",
                            action,selection,separate,n,i,((uint32_t *)map.data)[i],expected[i]); break;
                    }
                    REQUIRE(0);
                }
                TRY(ogpu_next_arena_reset(arena));
                ogpu_next_memory_destroy(preprocess); preprocess = NULL;
            }
            ogpu_next_generated_layout_destroy(layout); layout = NULL;
        }
    }
    printf("GENERATED_GRAPHICS_PASS submissions=%llu modes=%u actions=%u objects=32 pixels=%llu gpu_culling=1 gpu_compaction=1 gpu_count=1 selection_modes=%u sequence_identity=1 guards=1\n",
        (unsigned long long)value,selections*2,profiles,(unsigned long long)value*2048,selections-1);
    result = EXIT_SUCCESS;
cleanup:
    if (pending) { fprintf(stderr,"Pending generated graphics work after failure.\n"); _Exit(EXIT_FAILURE); }
    ogpu_next_arena_destroy(arena); ogpu_next_generated_layout_destroy(layout); ogpu_next_generated_set_destroy(set);
    ogpu_next_executable_destroy(producer); for (uint32_t i = 0; i < 2; ++i) ogpu_next_executable_destroy(pipelines[i]);
    ogpu_next_view_destroy(view); ogpu_next_image_destroy(image); ogpu_next_memory_destroy(backing);
    ogpu_next_timeline_destroy(done); ogpu_next_memory_destroy(preprocess); ogpu_next_memory_destroy(memory);
    ogpu_next_argument_layout_destroy(producer_arguments); ogpu_next_argument_layout_destroy(draw_arguments);
    if (file) fclose(file);
    for (uint32_t i = 0; i < 3; ++i) free(code[i]);
    free(bs); free(rs); free(ss);
    return result;
}
