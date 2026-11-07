/* Numerical extended-depth reference and static/dynamic native comparisons. */
static int depth_profiles_execution(ogpu_next_device *device,ogpu_next_memory_desc md,
                                 uint32_t domain,const char *const paths[2]) {
    int result=EXIT_FAILURE,pending=0;FILE *file=NULL;
    void *code[2]={NULL},*scratch=NULL,*render_scratch=NULL,*submit_scratch=NULL;
    ogpu_next_memory *memory=NULL,*backing[2]={NULL};ogpu_next_image *images[2]={NULL};ogpu_next_view *views[2]={NULL};
    ogpu_next_argument_layout *arguments=NULL;ogpu_next_executable *pipelines[3]={NULL};
    ogpu_next_arena *arena=NULL;ogpu_next_timeline *done=NULL;
    ogpu_next_feature_info features={0};ogpu_next_query q=query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));TRY(ogpu_next_device_query(device,&q));
    ogpu_next_argument_field field={0,8,OGPU_NEXT_ARGUMENT_INLINE,0,OGPU_NEXT_STAGE_VERTEX|OGPU_NEXT_STAGE_FRAGMENT,OGPU_NEXT_ARGUMENT_DIRECT,OGPU_NEXT_ARGUMENT_DIRECT};
    ogpu_next_argument_layout_desc ld={OGPU_NEXT_ARGUMENT_GRAPHICS,8,1,0,&field};TRY(ogpu_next_argument_layout_create(device,&ld,&arguments));
    ogpu_next_shader shaders[2];const char *names[]={"vertexMain","fragmentMain"};
    for(uint32_t i=0;i<2;++i){file=fopen(paths[i],"rb");REQUIRE(file);REQUIRE(fseek(file,0,SEEK_END)==0);long n=ftell(file);REQUIRE(n>=20&&n%4==0);REQUIRE(fseek(file,0,SEEK_SET)==0);
        code[i]=malloc((size_t)n);REQUIRE(code[i]);REQUIRE(fread(code[i],1,(size_t)n,file)==(size_t)n);fclose(file);file=NULL;
        shaders[i]=(ogpu_next_shader){i?OGPU_NEXT_STAGE_FRAGMENT:OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_SHADER_SPIRV,{code[i],(size_t)n},names[i],NULL,NULL};}
    TRY(ogpu_next_memory_create(device,&md,&memory));REQUIRE(md.size>=2048);ogpu_next_span all={memory,0,md.size};ogpu_next_mapping map={0};uint64_t address=0;
    TRY(ogpu_next_memory_map(all,&map));TRY(ogpu_next_memory_address(all,&address));
    for(uint32_t i=0;i<2;++i){uint32_t format=i?OGPU_NEXT_D32_FLOAT:OGPU_NEXT_RGBA8_UNORM;uint64_t usage=i?OGPU_NEXT_IMAGE_DEPTH_STENCIL_ATTACHMENT:OGPU_NEXT_IMAGE_COLOR_ATTACHMENT;
        ogpu_next_image_desc id={HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),format,OGPU_NEXT_IMAGE_2D,1,1,1,{8,8,1},usage|OGPU_NEXT_IMAGE_COPY_SRC,0,NULL,0,0,NULL};
        uint32_t types[32];ogpu_next_requirements ir={.compatible_types=types,.compatible_type_capacity=32};TRY(ogpu_next_image_requirements(device,&id,&ir));REQUIRE(ir.compatible_type_count);
        TRY(ogpu_next_image_create_unbound(device,&id,&images[i]));TRY(ogpu_next_memory_create_dedicated_image(images[i],types[0],&backing[i]));TRY(ogpu_next_image_bind(images[i],(ogpu_next_span){backing[i],0,ir.size}));
        ogpu_next_view_desc vd={HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),format,OGPU_NEXT_VIEW_2D,usage,{0},{i?OGPU_NEXT_ASPECT_DEPTH:OGPU_NEXT_ASPECT_COLOR,0,1,0,1}};TRY(ogpu_next_view_create(images[i],&vd,&views[i]));}
    ogpu_next_host_requirements br={0},rr={0},sr={0};TRY(ogpu_next_barrier_scratch_requirements(device,1,0,2,&br));TRY(ogpu_next_render_scratch_requirements(1,&rr));
    TRY(ogpu_next_submit_scratch_requirements(device,1,&(ogpu_next_submit_desc){.list_count=1,.signal_count=1},&sr));
    REQUIRE(br.alignment<=_Alignof(max_align_t)&&rr.alignment<=_Alignof(max_align_t)&&sr.alignment<=_Alignof(max_align_t));
    scratch=malloc((size_t)br.size);render_scratch=malloc((size_t)rr.size);submit_scratch=malloc((size_t)sr.size);REQUIRE(scratch&&render_scratch&&submit_scratch);
    ogpu_next_host_span bs={scratch,br.size};ogpu_next_arena_desc ad={HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};TRY(ogpu_next_arena_create(device,&ad,&arena));TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_recording_desc rd={HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    uint64_t serial=0,pixels=0;uint32_t tested=0,rejected=0;
    for(uint32_t mode=0;mode<5;++mode){
        const uint32_t requirements[5][4]={{OGPU_NEXT_FEATURE_UNRESTRICTED_DEPTH_RANGE,0,0,0},{OGPU_NEXT_FEATURE_UNRESTRICTED_DEPTH_RANGE,OGPU_NEXT_FEATURE_DEPTH_BOUNDS,0,0},
            {OGPU_NEXT_FEATURE_DEPTH_CLAMP_CONTROL,OGPU_NEXT_FEATURE_DEPTH_CLAMP,0,0},
            {OGPU_NEXT_FEATURE_DEPTH_BIAS_CONTROL,OGPU_NEXT_FEATURE_DEPTH_BIAS_FLOAT,OGPU_NEXT_FEATURE_DEPTH_BIAS_EXACT,0},
            {OGPU_NEXT_FEATURE_DEPTH_BIAS_CONTROL,OGPU_NEXT_FEATURE_DEPTH_BIAS_UNORM,OGPU_NEXT_FEATURE_DEPTH_BIAS_EXACT,0}};
        uint32_t available=1;for(uint32_t n=0;n<4;++n)if(requirements[mode][n]&&!raster_has(features.enabled,requirements[mode][n]))available=0;
        if(!available){printf("Depth profiles mode %u UNVERIFIED: unavailable feature.\n",mode);continue;}
        uint64_t dynamic=mode==1?OGPU_NEXT_DYNAMIC_DEPTH_BOUNDS:mode==2?OGPU_NEXT_DYNAMIC_DEPTH_CLAMP_RANGE:mode>=3?OGPU_NEXT_DYNAMIC_DEPTH_BIAS:0;
        ogpu_next_encoder *bad=NULL;ogpu_next_list *bad_list=NULL;TRY(ogpu_next_commands_begin(arena,&rd,&bad));
        if(mode==0){ogpu_next_viewport invalid={0,0,8,8,NAN,2};ogpu_next_set_viewports(bad,0,1,&invalid,0);}
        if(mode==1)ogpu_next_set_depth_bounds(bad,2,-1);
        if(mode==2)ogpu_next_set_depth_clamp_range(bad,OGPU_NEXT_CLAMP_USER_RANGE,.8f,.2f);
        if(mode==3)ogpu_next_set_depth_bias_control(bad,0,0,0,3,0);
        if(mode==4)ogpu_next_set_depth_bias_control(bad,0,0,0,0,2);
        REQUIRE(ogpu_next_commands_end(bad,&bad_list)!=OGPU_NEXT_OK&&!bad_list);TRY(ogpu_next_arena_reset(arena));++rejected;
        for(uint32_t i=0;i<3;++i){uint32_t phase=i==1;
            ogpu_next_raster_options o={.header=HEADER(ogpu_next_raster_options,OGPU_NEXT_RASTER_OPTIONS),.line_width=1,.max_depth_bounds=1,.sample_mask=UINT64_MAX};
            o.depth_bounds=mode==1;o.min_depth_bounds=mode==1?(phase?-2:-1):0;o.max_depth_bounds=mode==1?(phase?-1:2):1;
            o.depth_clamp=mode==2;o.depth_bias=mode>=3;o.depth_bias_constant=mode>=3?(phase?-1:1)*(mode==3?.125f:1048576):0;
            ogpu_next_depth_clamp_control clamp={HEADER(ogpu_next_depth_clamp_control,OGPU_NEXT_DEPTH_CLAMP_CONTROL_STATE),OGPU_NEXT_CLAMP_USER_RANGE,0,phase?.6f:.2f,phase?.8f:.4f};
            ogpu_next_depth_bias_representation bias={HEADER(ogpu_next_depth_bias_representation,OGPU_NEXT_DEPTH_BIAS_REPRESENTATION),mode==3?OGPU_NEXT_BIAS_FLOAT:OGPU_NEXT_BIAS_UNORM,1};
            if(mode==2){o.header.next=&clamp.header;}if(mode>=3){o.header.next=&bias.header;}
            if(i==2){if(mode==1)o.min_depth_bounds=o.max_depth_bounds=NAN;if(mode==2){clamp.mode=UINT32_MAX;clamp.min=clamp.max=NAN;}if(mode>=3){o.depth_bias_constant=NAN;bias.representation=bias.exact=UINT32_MAX;}}
            ogpu_next_color_state color={.format=OGPU_NEXT_RGBA8_UNORM,.write_mask=15};
            ogpu_next_graphics_state gs={.header=HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),.topology=OGPU_NEXT_TRIANGLES,.samples=1,.color_count=1,.colors=&color,
                .depth_format=OGPU_NEXT_D32_FLOAT,.depth_test=1,.depth_write=mode!=1,.depth_compare=OGPU_NEXT_COMPARE_LESS,.raster_options=&o.header};
            ogpu_next_shader_requirements req={HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),features.enabled,{0},0};
            ogpu_next_executable_desc ed={HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_GRAPHICS,2,shaders,&gs.header,OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR|(i==2?dynamic:0),&req.header,NULL,0,0};
            TRY(ogpu_next_executable_create(device,&ed,arguments,&pipelines[i]));}
        unsigned char reference[2][512];
        for(uint32_t path=0;path<2;++path){ogpu_next_encoder *e=NULL;ogpu_next_list *list=NULL;TRY(ogpu_next_commands_begin(arena,&rd,&e));
            ogpu_next_image_barrier ib[2];
            for(uint32_t i=0;i<2;++i)ib[i]=(ogpu_next_image_barrier){{0,OGPU_NEXT_STAGE_COLOR|OGPU_NEXT_STAGE_DEPTH,0,OGPU_NEXT_ACCESS_COLOR_WRITE|OGPU_NEXT_ACCESS_DEPTH_READ|OGPU_NEXT_ACCESS_DEPTH_WRITE},images[i],
                {i?OGPU_NEXT_ASPECT_DEPTH:OGPU_NEXT_ASPECT_COLOR,0,1,0,1},OGPU_NEXT_STATE_UNDEFINED,i?OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT:OGPU_NEXT_STATE_COLOR_ATTACHMENT,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1,0,NULL};
            ogpu_next_dependency dep={0,0,2,0,NULL,NULL,ib};ogpu_next_barrier(e,&dep,bs);
            ogpu_next_attachment ca={views[0],NULL,OGPU_NEXT_STATE_COLOR_ATTACHMENT,0,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.f32={0,0,0,1}}};
            ogpu_next_attachment da={views[1],NULL,OGPU_NEXT_STATE_DEPTH_STENCIL_ATTACHMENT,0,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.depth_stencil={mode==1?.5f:1,0}}};
            ogpu_next_render_desc render={.header=HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),.width=8,.height=8,.layers=1,.samples=1,.color_count=1,.colors=&ca,.depth=&da,.scratch={render_scratch,rr.size}};ogpu_next_render_begin(e,&render);
            if(path)ogpu_next_bind_executable(e,pipelines[2]);
            for(uint32_t phase=0;phase<2;++phase){if(!path)ogpu_next_bind_executable(e,pipelines[phase]);
                ogpu_next_viewport_state vp={HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),0,0,8,8,mode==0?(phase?-1:-.5f):0,mode==0?(phase?3:1.5f):1,4*phase,0,4,8};ogpu_next_set_graphics_state(e,&vp.header);
                uint64_t root=address+32*phase;ogpu_next_set_arguments(e,arguments,0,8,&root);
                if(path){if(mode==1)ogpu_next_set_depth_bounds(e,phase?-2:-1,phase?-1:2);
                    if(mode==2)ogpu_next_set_depth_clamp_range(e,OGPU_NEXT_CLAMP_USER_RANGE,phase?.6f:.2f,phase?.8f:.4f);
                    if(mode>=3)ogpu_next_set_depth_bias_control(e,(phase?-1:1)*(mode==3?.125f:1048576),0,0,mode==3?OGPU_NEXT_BIAS_FLOAT:OGPU_NEXT_BIAS_UNORM,1);}
                ogpu_next_draw_desc draw={6,1,0,0,0};ogpu_next_draw(e,&draw);
            }
            ogpu_next_render_end(e);
            for(uint32_t i=0;i<2;++i){ib[i].scope=(ogpu_next_barrier_scope){OGPU_NEXT_STAGE_COLOR|OGPU_NEXT_STAGE_DEPTH,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_ACCESS_COLOR_WRITE|OGPU_NEXT_ACCESS_DEPTH_WRITE,OGPU_NEXT_ACCESS_COPY_READ};ib[i].old_state=ib[i].new_state;ib[i].new_state=OGPU_NEXT_STATE_COPY_SRC;ib[i].discard=0;}
            ogpu_next_barrier(e,&dep,bs);
            for(uint32_t i=0;i<2;++i){ogpu_next_image_copy copy={{i?OGPU_NEXT_ASPECT_DEPTH:OGPU_NEXT_ASPECT_COLOR,0,0,1,{0,0,0},{8,8,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};ogpu_next_copy_from_image(e,(ogpu_next_span){memory,1024+256*i,256},images[i],&copy);}
            ogpu_next_barrier_scope host={OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_HOST,OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_HOST_READ};dep=(ogpu_next_dependency){1,0,0,0,&host,NULL,NULL};ogpu_next_barrier(e,&dep,bs);TRY(ogpu_next_commands_end(e,&list));
            for(uint32_t replay=0;replay<2;++replay){memset(map.data,0xa5,(size_t)md.size);
                for(uint32_t phase=0;phase<2;++phase){struct {uint32_t shape;float depth,slope;uint32_t unused;} c={0,mode==2?(replay?.75f:.5f):(replay?.375f:.25f),0,0};memcpy((char*)map.data+32*phase,&c,sizeof(c));}
                unsigned char before[1024];memcpy(before,map.data,1024);
                TRY(ogpu_next_memory_flush(all));ogpu_next_sync_point signal={{done,++serial},OGPU_NEXT_STAGE_ALL};ogpu_next_submit_desc submit={1,0,1,0,0,0,&list,NULL,&signal,NULL,NULL};ogpu_next_submit_outcome outcome={0};pending=1;
                TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),1,&submit,&outcome,(ogpu_next_host_span){submit_scratch,sr.size}));TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000)));pending=0;TRY(ogpu_next_memory_invalidate(all));
                
                for(uint32_t y=0;y<8;++y)for(uint32_t x=0;x<8;++x){uint32_t phase=x/4;unsigned char *c=(unsigned char*)map.data+1024+4*(y*8+x);uint32_t visible=c[0]==255;
                    REQUIRE((c[0]==0||c[0]==255)&&c[0]==c[1]&&c[1]==c[2]&&c[3]==255);
                    REQUIRE(visible==!(mode==1&&phase));
                    float depth;memcpy(&depth,(char*)map.data+1280+4*(y*8+x),4);float base=replay?.375f:.25f,expected=base;
                    if(mode==0)expected=(phase?-1:-.5f)+base*(phase?4:2);
                    if(mode==1)expected=.5f;
                    if(mode==2){expected=replay?.75f:.5f;if(expected<(phase?.6f:.2f))expected=phase?.6f:.2f;if(expected>(phase?.8f:.4f))expected=phase?.8f:.4f;}
                    if(mode>=3)expected+=(phase?-1:1)*(mode==3?.125f:.0625f);
                    if(fabsf(depth-expected)>1e-6f)fprintf(stderr,"Depth profiles %u path %u replay %u pixel %u,%u depth=%g expected=%g\n",mode,path,replay,x,y,depth,expected);
                    REQUIRE(fabsf(depth-expected)<=1e-6f);++pixels;}
                if(!path)memcpy(reference[replay],(char*)map.data+1024,512);else REQUIRE(!memcmp(reference[replay],(char*)map.data+1024,512));
                REQUIRE(!memcmp(before,map.data,1024));for(uint64_t i=1536;i<md.size;++i)REQUIRE(((unsigned char*)map.data)[i]==0xa5);
            }
            TRY(ogpu_next_arena_reset(arena));
        }
        for(uint32_t i=0;i<3;++i){ogpu_next_executable_destroy(pipelines[i]);pipelines[i]=NULL;}++tested;printf("Depth profiles mode %u PASS: static/dynamic bytes, changed-data replay, guards.\n",mode);
    }
    printf("Depth profiles PASS: %u profiles, %" PRIu64 " submissions, %" PRIu64 " color/depth pairs, %u rejected recordings.\n",tested,serial,pixels,rejected);result=EXIT_SUCCESS;
cleanup:
    if(pending){fputs("Pending depth-profile work after failure.\n",stderr);_Exit(EXIT_FAILURE);}
    ogpu_next_arena_destroy(arena);ogpu_next_timeline_destroy(done);for(uint32_t i=0;i<3;++i)ogpu_next_executable_destroy(pipelines[i]);ogpu_next_argument_layout_destroy(arguments);
    for(uint32_t i=0;i<2;++i){ogpu_next_view_destroy(views[i]);ogpu_next_image_destroy(images[i]);ogpu_next_memory_destroy(backing[i]);free(code[i]);}ogpu_next_memory_destroy(memory);
    if(file){fclose(file);}free(scratch);free(render_scratch);free(submit_scratch);return result;
}
