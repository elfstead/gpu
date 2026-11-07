/* Native point size, independent clipping, restart and instance fetch controls. */
static int raster_controls_execution(ogpu_next_device *device,ogpu_next_memory_desc md,
                                     uint32_t domain,const char *const paths[6]) {
    int result=EXIT_FAILURE,pending=0;FILE *file=NULL;
    void *code[6]={NULL},*scratch=NULL,*render_scratch=NULL,*submit_scratch=NULL,*vertex_scratch=NULL,*input_scratch=NULL;
    ogpu_next_memory *memory=NULL,*backing=NULL;ogpu_next_image *image=NULL;ogpu_next_view *view=NULL;
    ogpu_next_argument_layout *arguments=NULL;ogpu_next_executable *pipeline=NULL;
    ogpu_next_arena *arena=NULL;ogpu_next_timeline *done=NULL;ogpu_next_query_pool *statistics=NULL;
    ogpu_next_feature_info features={0};ogpu_next_query q=query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));TRY(ogpu_next_device_query(device,&q));
    ogpu_next_raster_control_limits limits={0};q=query(OGPU_NEXT_QUERY_RASTER_CONTROL_LIMITS,&limits,1,sizeof(limits));TRY(ogpu_next_device_query(device,&q));
    printf("Raster control limits: point=%g..%g granularity=%g divisor=%u first-instance=%u polygon-point=%u.\n",
        limits.point_size_range[0],limits.point_size_range[1],limits.point_size_granularity,limits.max_vertex_divisor,limits.nonzero_first_instance,limits.polygon_mode_point_size);
    uint64_t stages=OGPU_NEXT_STAGE_VERTEX|OGPU_NEXT_STAGE_FRAGMENT;
    if(raster_has(features.enabled,OGPU_NEXT_FEATURE_TESSELLATION))stages|=OGPU_NEXT_STAGE_TESS_CONTROL|OGPU_NEXT_STAGE_TESS_EVALUATION;
    ogpu_next_argument_field field={0,8,OGPU_NEXT_ARGUMENT_INLINE,0,stages,OGPU_NEXT_ARGUMENT_DIRECT,OGPU_NEXT_ARGUMENT_DIRECT};
    ogpu_next_argument_layout_desc ld={OGPU_NEXT_ARGUMENT_GRAPHICS,8,1,0,&field};TRY(ogpu_next_argument_layout_create(device,&ld,&arguments));
    const char *names[]={"vertexMain","pointMain","fetchMain","hullMain","domainMain","fragmentMain"};
    const uint32_t shader_stages[]={OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_STAGE_TESS_CONTROL,OGPU_NEXT_STAGE_TESS_EVALUATION,OGPU_NEXT_STAGE_FRAGMENT};
    ogpu_next_shader shaders[6];
    for(uint32_t i=0;i<6;++i) {
        file=fopen(paths[i],"rb");REQUIRE(file);REQUIRE(fseek(file,0,SEEK_END)==0);long n=ftell(file);
        REQUIRE(n>=20 && n%4==0);REQUIRE(fseek(file,0,SEEK_SET)==0);code[i]=malloc((size_t)n);REQUIRE(code[i]);
        REQUIRE(fread(code[i],1,(size_t)n,file)==(size_t)n);fclose(file);file=NULL;
        shaders[i]=(ogpu_next_shader){shader_stages[i],OGPU_NEXT_SHADER_SPIRV,{code[i],(size_t)n},names[i],NULL,NULL};
    }
    TRY(ogpu_next_memory_create(device,&md,&memory)); REQUIRE(md.size>=2048);
    ogpu_next_span all={memory,0,md.size};ogpu_next_mapping map={0};uint64_t address=0;
    TRY(ogpu_next_memory_map(all,&map));TRY(ogpu_next_memory_address(all,&address));
    ogpu_next_image_desc id={HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),OGPU_NEXT_RGBA8_UNORM,OGPU_NEXT_IMAGE_2D,1,1,1,{8,8,1},OGPU_NEXT_IMAGE_COLOR_ATTACHMENT|OGPU_NEXT_IMAGE_COPY_SRC,0,NULL,0,0,NULL};
    uint32_t types[32];ogpu_next_requirements ir={.compatible_types=types,.compatible_type_capacity=32};TRY(ogpu_next_image_requirements(device,&id,&ir));REQUIRE(ir.compatible_type_count);
    TRY(ogpu_next_image_create_unbound(device,&id,&image));TRY(ogpu_next_memory_create_dedicated_image(image,types[0],&backing));TRY(ogpu_next_image_bind(image,(ogpu_next_span){backing,0,ir.size}));
    ogpu_next_subresources range={OGPU_NEXT_ASPECT_COLOR,0,1,0,1};
    ogpu_next_view_desc vd={HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),OGPU_NEXT_RGBA8_UNORM,OGPU_NEXT_VIEW_2D,OGPU_NEXT_IMAGE_COLOR_ATTACHMENT,{0},range};TRY(ogpu_next_view_create(image,&vd,&view));
    ogpu_next_host_requirements vr={0};TRY(ogpu_next_vertex_scratch_requirements(1,&vr));REQUIRE(vr.alignment<=_Alignof(max_align_t));
    vertex_scratch=malloc((size_t)vr.size);REQUIRE(vertex_scratch);
    ogpu_next_host_requirements vi={0};TRY(ogpu_next_vertex_input_scratch_requirements(1,1,&vi));REQUIRE(vi.alignment<=_Alignof(max_align_t));
    input_scratch=malloc((size_t)vi.size);REQUIRE(input_scratch);
    ogpu_next_host_requirements br={0},rr={0},sr={0};TRY(ogpu_next_barrier_scratch_requirements(device,1,0,1,&br));TRY(ogpu_next_render_scratch_requirements(1,&rr));
    TRY(ogpu_next_submit_scratch_requirements(device,1,&(ogpu_next_submit_desc){.list_count=1,.signal_count=1},&sr));
    REQUIRE(br.alignment<=_Alignof(max_align_t) && rr.alignment<=_Alignof(max_align_t) && sr.alignment<=_Alignof(max_align_t));
    scratch=malloc((size_t)br.size);render_scratch=malloc((size_t)rr.size);submit_scratch=malloc((size_t)sr.size);REQUIRE(scratch && render_scratch && submit_scratch);
    ogpu_next_host_span bs={scratch,br.size};
    ogpu_next_arena_desc ad={HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};TRY(ogpu_next_arena_create(device,&ad,&arena));TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_recording_desc rd={HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    uint64_t serial=0,checked_pixels=0;uint32_t tested=0,rejected=0,rejected_recordings=0;
    for(uint32_t mode=0;mode<17;++mode) {
        uint32_t point=mode==0,clip=mode>=1&&mode<=4,restart=mode==5||mode==6,tess=mode==6,fetch=mode>=7;
        uint32_t dynamic_input=mode>=11,dynamic=mode==9||mode==10||mode==13;
        uint32_t divisor=mode==8||mode==10||mode==12?0:mode==13||mode==14||mode==15?1:2;
        uint32_t first=(dynamic||dynamic_input)&&limits.nonzero_first_instance&&mode!=15?1:0;
        uint32_t binding_id=mode==12||mode==14?5:3,offset=mode==12||mode==13||mode==14?16:mode==16?4:0;
        uint32_t stride=mode==14?0:mode==16?8:(dynamic||mode==12)?32:16;
        uint32_t clipping=clip?((mode-1)&1):0,clamp=clip?(mode-1)/2:0;
        if((point&&(!raster_has(features.enabled,OGPU_NEXT_FEATURE_LARGE_POINTS)||limits.point_size_range[1]<6)) ||
           (clip&&!raster_has(features.enabled,OGPU_NEXT_FEATURE_DEPTH_CLIP)) ||
           (clamp&&!raster_has(features.enabled,OGPU_NEXT_FEATURE_DEPTH_CLAMP)) ||
           (mode==5&&!raster_has(features.enabled,OGPU_NEXT_FEATURE_LIST_RESTART)) ||
           (tess&&(!raster_has(features.enabled,OGPU_NEXT_FEATURE_PATCH_RESTART)||!raster_has(features.enabled,OGPU_NEXT_FEATURE_TESSELLATION))) ||
           (fetch&&divisor!=1&&!raster_has(features.enabled,divisor?OGPU_NEXT_FEATURE_VERTEX_DIVISOR:OGPU_NEXT_FEATURE_VERTEX_ZERO_DIVISOR)) ||
           (dynamic_input&&!raster_has(features.enabled,OGPU_NEXT_FEATURE_DYNAMIC_VERTEX_INPUT)) ||
           (mode==15&&(!limits.nonzero_first_instance||!raster_has(features.enabled,OGPU_NEXT_FEATURE_VERTEX_ZERO_DIVISOR))) ||
           (((dynamic_input&&divisor==0)||mode==15)&&!raster_has(features.enabled,OGPU_NEXT_FEATURE_VERTEX_DIVISOR))) {
            printf("Raster controls mode %u UNVERIFIED: unsupported native profile.\n",mode);continue;
        }
        ogpu_next_shader selected[4];uint32_t count=0;selected[count++]=shaders[point?1:fetch?2:0];
        if(tess){selected[count++]=shaders[3];selected[count++]=shaders[4];}selected[count++]=shaders[5];
        ogpu_next_shader_requirements requirements={HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),features.enabled,{0},0};
        ogpu_next_color_state color={.format=OGPU_NEXT_RGBA8_UNORM,.write_mask=15};
        ogpu_next_depth_clip_state cs={HEADER(ogpu_next_depth_clip_state,OGPU_NEXT_DEPTH_CLIP_STATE),clipping,0};
        ogpu_next_raster_options options={.header=HEADER(ogpu_next_raster_options,OGPU_NEXT_RASTER_OPTIONS),.primitive_restart=restart,.depth_clamp=clamp,.line_width=1,.max_depth_bounds=1,.sample_mask=UINT64_MAX};
        if(clip)options.header.next=&cs.header;
        ogpu_next_vertex_divisor entry={binding_id,divisor};
        ogpu_next_vertex_divisors divisors={HEADER(ogpu_next_vertex_divisors,OGPU_NEXT_VERTEX_DIVISORS),1,0,&entry};
        ogpu_next_vertex_binding binding={binding_id,dynamic_input?stride:16,mode==14?OGPU_NEXT_VERTEX_RATE_VERTEX:OGPU_NEXT_VERTEX_RATE_INSTANCE};
        ogpu_next_vertex_attribute attribute={0,binding_id,mode==16?OGPU_NEXT_RGBA8_UNORM:OGPU_NEXT_RGBA32_FLOAT,offset};
        ogpu_next_vertex_input input={HEADER(ogpu_next_vertex_input,OGPU_NEXT_VERTEX_INPUT),1,1,&binding,&attribute};input.header.next=mode==14?NULL:&divisors.header;
        ogpu_next_tessellation_state ts={HEADER(ogpu_next_tessellation_state,OGPU_NEXT_TESSELLATION_STATE),3,OGPU_NEXT_DOMAIN_UPPER_LEFT};
        ogpu_next_graphics_state gs={.header=HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),.topology=point?OGPU_NEXT_POINTS:tess?OGPU_NEXT_PATCHES:OGPU_NEXT_TRIANGLES,
            .front_face=OGPU_NEXT_FRONT_CCW,.samples=1,.color_count=1,.colors=&color,.raster_options=&options.header,.vertex_input=fetch?&input.header:NULL};
        if(tess)gs.header.next=&ts.header;
        ogpu_next_executable_desc ed={HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_GRAPHICS,count,selected,&gs.header,
            OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR|(dynamic?OGPU_NEXT_DYNAMIC_VERTEX_STRIDE:0)|(dynamic_input?OGPU_NEXT_DYNAMIC_VERTEX_INPUT:0),&requirements.header,NULL,0,0};
        if(mode==1||mode==7) {
            for(uint32_t bad=0;bad<3;++bad) {
                if(clip){cs.enable=bad==0?2:0;cs.reserved=bad==1;cs.header.next=bad==2?&cs.header:NULL;}
                else {entry.binding=bad==0?4:3;binding.rate=bad==1?OGPU_NEXT_VERTEX_RATE_VERTEX:OGPU_NEXT_VERTEX_RATE_INSTANCE;divisors.reserved=bad==2;}
                ogpu_next_executable *invalid=NULL;REQUIRE(ogpu_next_executable_create(device,&ed,arguments,&invalid)!=OGPU_NEXT_OK);REQUIRE(!invalid);++rejected;
            }
            cs.enable=clipping;cs.reserved=0;cs.header.next=NULL;entry.binding=3;binding.rate=OGPU_NEXT_VERTEX_RATE_INSTANCE;divisors.reserved=0;
        }
        ogpu_next_vertex_input ignored={.header=HEADER(ogpu_next_vertex_input,UINT32_MAX),.binding_count=UINT32_MAX};
        if(dynamic_input)gs.vertex_input=&ignored.header; /* Entire static input must be ignored. */
        TRY(ogpu_next_executable_create(device,&ed,arguments,&pipeline));
        if(mode==11) {
            for(uint32_t bad=0;bad<7;++bad) {
                ogpu_next_vertex_input invalid=input;ogpu_next_vertex_attribute wrong=attribute;
                ogpu_next_host_span work={input_scratch,vi.size};
                if(bad==0)invalid.binding_count=UINT32_MAX;
                if(bad==1){wrong.binding=4;invalid.attributes=&wrong;}
                if(bad==2){wrong.format=OGPU_NEXT_D32_FLOAT;invalid.attributes=&wrong;}
                if(bad==3)work.size--;
                if(bad==4)work.data=NULL;
                if(bad==5)divisors.reserved=1;
                if(bad==6)binding.rate=OGPU_NEXT_VERTEX_RATE_VERTEX;
                ogpu_next_encoder *bad_encoder=NULL;ogpu_next_list *bad_list=NULL;
                TRY(ogpu_next_commands_begin(arena,&rd,&bad_encoder));
                ogpu_next_set_vertex_input(bad_encoder,&invalid,work);
                REQUIRE(ogpu_next_commands_end(bad_encoder,&bad_list)!=OGPU_NEXT_OK);REQUIRE(!bad_list);
                TRY(ogpu_next_arena_reset(arena));++rejected_recordings;
                divisors.reserved=0;binding.rate=OGPU_NEXT_VERTEX_RATE_INSTANCE;
            }
        }
        if(restart&&raster_has(features.enabled,OGPU_NEXT_FEATURE_PIPELINE_STATISTICS)) {
            ogpu_next_query_pool_desc qp={HEADER(ogpu_next_query_pool_desc,OGPU_NEXT_QUERY_POOL_DESC),OGPU_NEXT_QUERY_PIPELINE_STATISTICS,1,tess?OGPU_NEXT_STATISTIC_TESS_CONTROL_PATCHES:OGPU_NEXT_STATISTIC_INPUT_PRIMITIVES};
            TRY(ogpu_next_query_pool_create(device,&qp,&statistics));
        }
        ogpu_next_encoder *e=NULL;ogpu_next_list *list=NULL;TRY(ogpu_next_commands_begin(arena,&rd,&e));
        if(statistics)ogpu_next_queries_reset(e,statistics,0,1);
        ogpu_next_image_barrier ib={{0,OGPU_NEXT_STAGE_COLOR,0,OGPU_NEXT_ACCESS_COLOR_WRITE},image,range,OGPU_NEXT_STATE_UNDEFINED,OGPU_NEXT_STATE_COLOR_ATTACHMENT,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1,0,NULL};
        ogpu_next_dependency dep={0,0,1,0,NULL,NULL,&ib};ogpu_next_barrier(e,&dep,bs);
        ogpu_next_attachment a={view,NULL,OGPU_NEXT_STATE_COLOR_ATTACHMENT,0,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.f32={0,0,0,1}}};
        ogpu_next_render_desc render={.header=HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),.width=8,.height=8,.layers=1,.samples=1,.color_count=1,.colors=&a,.scratch={render_scratch,rr.size}};
        ogpu_next_viewport_state vp={HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),0,0,8,8,0,1,0,0,8,8};
        ogpu_next_render_begin(e,&render);ogpu_next_bind_executable(e,pipeline);ogpu_next_set_graphics_state(e,&vp.header);ogpu_next_set_arguments(e,arguments,0,8,&address);
        if(dynamic_input) {
            ogpu_next_vertex_input empty={.header=HEADER(ogpu_next_vertex_input,OGPU_NEXT_VERTEX_INPUT)};
            ogpu_next_set_vertex_input(e,&empty,(ogpu_next_host_span){0});
            if(mode==13)binding.stride=16; /* Later address bind overrides this to 32. */
            ogpu_next_set_vertex_input(e,&input,(ogpu_next_host_span){input_scratch,vi.size});
        }
        if(fetch){ogpu_next_vertex_buffer_binding vb={{memory,128,192},dynamic?32u:0u,dynamic?OGPU_NEXT_VERTEX_BIND_STRIDE:0};ogpu_next_bind_vertices(e,binding_id,1,&vb,(ogpu_next_host_span){vertex_scratch,vr.size});}
        if(statistics)ogpu_next_query_begin(e,statistics,0,0,0);
        ogpu_next_draw_desc draw={point?1u:restart?5u:fetch?6u:3u,fetch?4u:1u,0,first,0};
        if(restart){ogpu_next_bind_indices(e,(ogpu_next_span){memory,320,20},OGPU_NEXT_INDEX_U32);ogpu_next_draw_indexed(e,&draw);}else if(mode==15) {
            draw.instances=2;ogpu_next_draw(e,&draw);
            entry.divisor=0;ogpu_next_set_vertex_input(e,&input,(ogpu_next_host_span){input_scratch,vi.size});
            draw.first_instance=2;ogpu_next_draw(e,&draw); /* Same executable, different divisor. */
        } else ogpu_next_draw(e,&draw);
        if(statistics)ogpu_next_query_end(e,statistics,0,0);
        ogpu_next_render_end(e);
        ib=(ogpu_next_image_barrier){{OGPU_NEXT_STAGE_COLOR,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_ACCESS_COLOR_WRITE,OGPU_NEXT_ACCESS_COPY_READ},image,range,OGPU_NEXT_STATE_COLOR_ATTACHMENT,OGPU_NEXT_STATE_COPY_SRC,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,0,0,NULL};ogpu_next_barrier(e,&dep,bs);
        ogpu_next_image_copy copy={{OGPU_NEXT_ASPECT_COLOR,0,0,1,{0,0,0},{8,8,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};ogpu_next_copy_from_image(e,(ogpu_next_span){memory,1024,256},image,&copy);
        if(statistics)ogpu_next_queries_resolve(e,statistics,0,1,(ogpu_next_span){memory,512,8},8,OGPU_NEXT_QUERY_RESULT_64|OGPU_NEXT_QUERY_RESULT_WAIT);
        ogpu_next_barrier_scope host={OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_HOST,OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_HOST_READ};dep=(ogpu_next_dependency){1,0,0,0,&host,NULL,NULL};ogpu_next_barrier(e,&dep,bs);
        TRY(ogpu_next_commands_end(e,&list));
        for(uint32_t replay=0;replay<2;++replay) {
            memset(map.data,0xa5,(size_t)md.size);
            struct {float depth,size;uint32_t first,unused;} control={replay?.5f:-.5f,4.0f+2*replay,first,0};
            if(restart)control.depth=0;
            memcpy(map.data,&control,sizeof(control));uint32_t indices[5]={0,UINT32_MAX,0,1,2};memcpy((char*)map.data+320,indices,20);
            const float palette[4][4]={{1,0,0,1},{0,1,0,1},{0,0,1,1},{1,1,0,1}};
            for(uint32_t i=0;i<(mode==14?1u:mode==13?5u:4u);++i) {
                void *dest=(char*)map.data+128+i*stride+offset;
                if(mode==16){for(uint32_t c=0;c<4;++c)((unsigned char*)dest)[c]=(unsigned char)(255*palette[(i+replay)%4][c]);}
                else memcpy(dest,palette[(i+replay)%4],16);
            }
            unsigned char before[1024];memcpy(before,map.data,sizeof(before));
            TRY(ogpu_next_memory_flush(all));ogpu_next_sync_point signal={{done,++serial},OGPU_NEXT_STAGE_ALL};ogpu_next_submit_desc submit={1,0,1,0,0,0,&list,NULL,&signal,NULL,NULL};ogpu_next_submit_outcome outcome={0};pending=1;
            TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),1,&submit,&outcome,(ogpu_next_host_span){submit_scratch,sr.size}));TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000)));pending=0;TRY(ogpu_next_memory_invalidate(all));
            for(uint32_t y=0;y<8;++y)for(uint32_t x=0;x<8;++x) {
                uint32_t visible=point?(x>=2-replay&&x<6+replay&&y>=2-replay&&y<6+replay):!(clip&&clipping&&!replay);
                uint32_t fetched=mode==14?0:mode==15?(x<4?x/2:2):first+(divisor?(x/2)/divisor:0);
                const float *color_value=fetch?palette[(fetched+replay)%4]:NULL;
                unsigned char *pixel=(unsigned char*)map.data+1024+4*(y*8+x);
                for(uint32_t channel=0;channel<4;++channel){uint32_t expected=channel==3?255:fetch?(uint32_t)(255*color_value[channel]):visible?255:0;
                    if(pixel[channel]!=expected){fprintf(stderr,"Raster control mode %u replay %u at %u,%u channel %u: %u expected %u\n",mode,replay,x,y,channel,pixel[channel],expected);}REQUIRE(pixel[channel]==expected);}
                ++checked_pixels;
            }
            if(statistics)REQUIRE(*(uint64_t*)((char*)map.data+512)==1);
            for(uint32_t i=0;i<1024;++i)if(!statistics||i<512||i>=520)REQUIRE(((unsigned char*)map.data)[i]==before[i]);
            for(uint64_t i=1280;i<md.size;++i)REQUIRE(((unsigned char*)map.data)[i]==0xa5);
        }
        printf("Raster controls mode %u PASS: divisor=%u dynamic-stride=%u dynamic-input=%u first-instance=%u.\n",mode,fetch?divisor:1,dynamic,dynamic_input,first);++tested;
        TRY(ogpu_next_arena_reset(arena));ogpu_next_executable_destroy(pipeline);pipeline=NULL;ogpu_next_query_pool_destroy(statistics);statistics=NULL;
    }
    printf("Raster controls PASS: %u profiles, %" PRIu64 " submissions, %" PRIu64 " pixels, %u rejected preparations, %u rejected recordings and guards.\n",tested,serial,checked_pixels,rejected,rejected_recordings);result=EXIT_SUCCESS;
cleanup:
    if(pending){fputs("Pending raster-control work after failure.\n",stderr);_Exit(EXIT_FAILURE);}
    ogpu_next_arena_destroy(arena);ogpu_next_timeline_destroy(done);ogpu_next_query_pool_destroy(statistics);ogpu_next_executable_destroy(pipeline);ogpu_next_argument_layout_destroy(arguments);
    ogpu_next_view_destroy(view);ogpu_next_image_destroy(image);ogpu_next_memory_destroy(backing);ogpu_next_memory_destroy(memory);
    if(file){fclose(file);}for(uint32_t i=0;i<6;++i){free(code[i]);}free(vertex_scratch);free(input_scratch);free(scratch);free(render_scratch);free(submit_scratch);return result;
}
