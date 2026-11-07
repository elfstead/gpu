/* Indexed viewport arrays and independent count modes versus explicit slot-zero draws. */
static int viewport_execution(ogpu_next_device *device,ogpu_next_memory_desc md,uint32_t domain,const char *vertex_path,const char *fragment_path) {
    int result=EXIT_FAILURE,pending=0;FILE *file=NULL;
    void *code[2]={NULL},*bs=NULL,*rs=NULL,*ss=NULL;
    ogpu_next_memory *memory=NULL,*backing[4]={NULL};ogpu_next_image *images[4]={NULL};ogpu_next_view *views[4]={NULL};
    ogpu_next_argument_layout *arguments=NULL;ogpu_next_executable *pipelines[3]={NULL};ogpu_next_arena *arena=NULL;ogpu_next_timeline *done=NULL;
    ogpu_next_feature_info features={0};ogpu_next_query q=query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));TRY(ogpu_next_device_query(device,&q));
    if(!raster_has(features.enabled,OGPU_NEXT_FEATURE_MULTI_VIEWPORT)||!raster_has(features.enabled,OGPU_NEXT_FEATURE_SHADER_OUTPUT_VIEWPORT_INDEX)){puts("Viewport arrays UNVERIFIED: viewport capabilities unavailable.");return EXIT_SUCCESS;}
    ogpu_next_viewport_limits vl={0};q=query(OGPU_NEXT_QUERY_VIEWPORT_LIMITS,&vl,1,sizeof(vl));TRY(ogpu_next_device_query(device,&q));REQUIRE(vl.max_viewports>=2);
    ogpu_next_graphics_limits limits={0};q=query(OGPU_NEXT_QUERY_GRAPHICS_LIMITS,&limits,1,sizeof(limits));TRY(ogpu_next_device_query(device,&q));
    if(limits.max_colors<2||!(limits.color_samples&4)||!raster_has(features.enabled,OGPU_NEXT_FEATURE_INDEPENDENT_BLEND)){puts("Viewport arrays UNVERIFIED: two independent four-sample targets unavailable.");return EXIT_SUCCESS;}
    ogpu_next_argument_field field={0,8,OGPU_NEXT_ARGUMENT_INLINE,0,OGPU_NEXT_STAGE_VERTEX|OGPU_NEXT_STAGE_FRAGMENT,OGPU_NEXT_ARGUMENT_DIRECT,OGPU_NEXT_ARGUMENT_DIRECT};
    ogpu_next_argument_layout_desc ld={OGPU_NEXT_ARGUMENT_GRAPHICS,8,1,0,&field};TRY(ogpu_next_argument_layout_create(device,&ld,&arguments));
    ogpu_next_shader shaders[2];const char *paths[]={vertex_path,fragment_path};
    for(uint32_t i=0;i<2;++i){file=fopen(paths[i],"rb");REQUIRE(file);REQUIRE(fseek(file,0,SEEK_END)==0);long n=ftell(file);REQUIRE(n>=20&&n%4==0);REQUIRE(fseek(file,0,SEEK_SET)==0);
        code[i]=malloc((size_t)n);REQUIRE(code[i]);REQUIRE(fread(code[i],1,(size_t)n,file)==(size_t)n);fclose(file);file=NULL;
        shaders[i]=(ogpu_next_shader){i?OGPU_NEXT_STAGE_FRAGMENT:OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_SHADER_SPIRV,{code[i],(size_t)n},i?"fragmentMain":"vertexMain",NULL,NULL};}
    TRY(ogpu_next_memory_create(device,&md,&memory));REQUIRE(md.size>=2048);ogpu_next_span all={memory,0,md.size};ogpu_next_mapping map={0};uint64_t address=0;
    TRY(ogpu_next_memory_map(all,&map));TRY(ogpu_next_memory_address(all,&address));
    for(uint32_t i=0;i<4;++i){ogpu_next_image_desc id={HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),OGPU_NEXT_RGBA8_UNORM,OGPU_NEXT_IMAGE_2D,1,1,i<2?1:4,{8,8,1},OGPU_NEXT_IMAGE_COLOR_ATTACHMENT|OGPU_NEXT_IMAGE_COPY_SRC,0,NULL,0,0,NULL};
        uint32_t types[32];ogpu_next_requirements ir={.compatible_types=types,.compatible_type_capacity=32};TRY(ogpu_next_image_requirements(device,&id,&ir));REQUIRE(ir.compatible_type_count);
        TRY(ogpu_next_image_create_unbound(device,&id,&images[i]));TRY(ogpu_next_memory_create_dedicated_image(images[i],types[0],&backing[i]));TRY(ogpu_next_image_bind(images[i],(ogpu_next_span){backing[i],0,ir.size}));
        ogpu_next_view_desc vd={HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),OGPU_NEXT_RGBA8_UNORM,OGPU_NEXT_VIEW_2D,OGPU_NEXT_IMAGE_COLOR_ATTACHMENT,{0},{OGPU_NEXT_ASPECT_COLOR,0,1,0,1}};TRY(ogpu_next_view_create(images[i],&vd,&views[i]));}
    ogpu_next_host_requirements br={0},rr={0},sr={0};TRY(ogpu_next_barrier_scratch_requirements(device,1,0,4,&br));TRY(ogpu_next_render_scratch_requirements(2,&rr));
    TRY(ogpu_next_submit_scratch_requirements(device,1,&(ogpu_next_submit_desc){.list_count=1,.signal_count=1},&sr));
    REQUIRE(br.alignment<=_Alignof(max_align_t)&&rr.alignment<=_Alignof(max_align_t)&&sr.alignment<=_Alignof(max_align_t));
    bs=malloc((size_t)br.size);rs=malloc((size_t)rr.size);ss=malloc((size_t)sr.size);REQUIRE(bs&&rs&&ss);
    ogpu_next_host_span scratch={bs,br.size};ogpu_next_arena_desc ad={HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};TRY(ogpu_next_arena_create(device,&ad,&arena));TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_recording_desc rd={HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    uint64_t serial=0,pixels=0;uint32_t profiles=0,rejected=0;
    for(uint32_t mode=0;mode<5;++mode){
        uint64_t dynamic=(mode==1||mode==2||mode==4?OGPU_NEXT_DYNAMIC_VIEWPORT_COUNT:0)|(mode==1||mode==3||mode==4?OGPU_NEXT_DYNAMIC_SCISSOR_COUNT:0);
        ogpu_next_encoder *bad=NULL;ogpu_next_list *bad_list=NULL;TRY(ogpu_next_commands_begin(arena,&rd,&bad));
        ogpu_next_viewport invalid={0,0,8,0,0,1};ogpu_next_scissor invalid_scissor={-1,0,8,8};
        switch(mode){case 0:ogpu_next_set_viewports(bad,0,1,&invalid,0);break;case 1:ogpu_next_set_scissors(bad,0,1,&invalid_scissor,0);break;
        case 2:ogpu_next_set_viewports(bad,0,0,NULL,OGPU_NEXT_SET_COUNT);break;case 3:ogpu_next_set_scissors(bad,1,1,&invalid_scissor,OGPU_NEXT_SET_COUNT);break;
        default:ogpu_next_set_scissors(bad,UINT32_MAX,1,&invalid_scissor,0);break;}
        REQUIRE(ogpu_next_commands_end(bad,&bad_list)!=OGPU_NEXT_OK&&!bad_list);TRY(ogpu_next_arena_reset(arena));++rejected;
        for(uint32_t sample_pass=0;sample_pass<2;++sample_pass){uint32_t samples=sample_pass?4:1;
            for(uint32_t i=0;i<2;++i){
                ogpu_next_color_state c[2]={{.format=OGPU_NEXT_RGBA8_UNORM,.write_mask=15},{.format=OGPU_NEXT_RGBA8_UNORM,.write_mask=15}};
                ogpu_next_viewport_counts counts={HEADER(ogpu_next_viewport_counts,OGPU_NEXT_VIEWPORT_COUNTS),i?2:1,i?2:1};
                if(i&&dynamic&OGPU_NEXT_DYNAMIC_VIEWPORT_COUNT)counts.viewports=UINT32_MAX;
                if(i&&dynamic&OGPU_NEXT_DYNAMIC_SCISSOR_COUNT)counts.scissors=UINT32_MAX;
                ogpu_next_graphics_state gs={.header=HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),.topology=OGPU_NEXT_TRIANGLES,.samples=samples,.color_count=2,.colors=c};gs.header.next=&counts.header;
                ogpu_next_shader_requirements req={HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),features.enabled,{0},0};
                ogpu_next_executable_desc ed={HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_GRAPHICS,2,shaders,&gs.header,OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR|(i?dynamic:0),&req.header,NULL,0,0};
                TRY(ogpu_next_executable_create(device,&ed,arguments,&pipelines[i]));}
            unsigned char reference[2][512];
            for(uint32_t path=0;path<2;++path){ogpu_next_encoder *e=NULL;ogpu_next_list *list=NULL;TRY(ogpu_next_commands_begin(arena,&rd,&e));
                ogpu_next_image_barrier ib[4];for(uint32_t a=0;a<4;++a)ib[a]=(ogpu_next_image_barrier){{0,OGPU_NEXT_STAGE_COLOR,0,OGPU_NEXT_ACCESS_COLOR_READ|OGPU_NEXT_ACCESS_COLOR_WRITE},images[a],{OGPU_NEXT_ASPECT_COLOR,0,1,0,1},OGPU_NEXT_STATE_UNDEFINED,OGPU_NEXT_STATE_COLOR_ATTACHMENT,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1,0,NULL};
                ogpu_next_dependency dep={0,0,4,0,NULL,NULL,ib};ogpu_next_barrier(e,&dep,scratch);
                ogpu_next_attachment ca[2];for(uint32_t a=0;a<2;++a)ca[a]=(ogpu_next_attachment){views[a+(sample_pass?2:0)],sample_pass?views[a]:NULL,OGPU_NEXT_STATE_COLOR_ATTACHMENT,sample_pass?OGPU_NEXT_STATE_COLOR_ATTACHMENT:0,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,sample_pass?OGPU_NEXT_ATTACHMENT_RESOLVE_AVERAGE:0,{.f32={.5f,.25f,.125f,.75f}}};
                ogpu_next_render_desc render={.header=HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),.width=8,.height=8,.layers=1,.samples=samples,.color_count=2,.colors=ca,.scratch={rs,rr.size}};ogpu_next_render_begin(e,&render);
                ogpu_next_bind_executable(e,pipelines[path]);
                ogpu_next_viewport vp[2]={{0,0,4,8,0,1},{4,8,4,-8,1,0}};
                ogpu_next_scissor sc[2]={{0,0,4,8},{4,0,4,8}};
                uint64_t root=address+64*path;ogpu_next_set_arguments(e,arguments,0,8,&root);
                if(!path){for(uint32_t phase=0;phase<2;++phase){ogpu_next_set_viewports(e,0,1,&vp[phase],0);ogpu_next_set_scissors(e,0,1,&sc[phase],0);
                    ogpu_next_draw_desc draw={3,1,0,phase,0};ogpu_next_draw(e,&draw);}}
                else {
                    uint32_t vf=dynamic&OGPU_NEXT_DYNAMIC_VIEWPORT_COUNT?OGPU_NEXT_SET_COUNT:0,sf=dynamic&OGPU_NEXT_DYNAMIC_SCISSOR_COUNT?OGPU_NEXT_SET_COUNT:0;
                    if(mode==4){ogpu_next_set_viewports(e,0,1,vp,vf);ogpu_next_set_scissors(e,0,1,sc,sf);ogpu_next_draw_desc draw={3,1,0,0,0};ogpu_next_draw(e,&draw);}
                    if(vf)ogpu_next_set_viewports(e,0,2,vp,vf);else{ogpu_next_set_viewports(e,1,1,&vp[1],0);ogpu_next_set_viewports(e,0,1,&vp[0],0);}
                    if(sf)ogpu_next_set_scissors(e,0,2,sc,sf);else{ogpu_next_set_scissors(e,1,1,&sc[1],0);ogpu_next_set_scissors(e,0,1,&sc[0],0);}
                    /* Empty range updates must not invalidate established count state. */
                    ogpu_next_set_viewports(e,0,0,NULL,0);ogpu_next_set_scissors(e,0,0,NULL,0);
                    ogpu_next_draw_desc draw={3,mode==4?1:2,0,mode==4?1:0,0};ogpu_next_draw(e,&draw);
                }
                ogpu_next_render_end(e);
                for(uint32_t a=0;a<2;++a){ib[a].scope=(ogpu_next_barrier_scope){OGPU_NEXT_STAGE_COLOR,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_ACCESS_COLOR_WRITE,OGPU_NEXT_ACCESS_COPY_READ};ib[a].old_state=ib[a].new_state;ib[a].new_state=OGPU_NEXT_STATE_COPY_SRC;ib[a].discard=0;}
                dep.image_count=2;ogpu_next_barrier(e,&dep,scratch);
                for(uint32_t a=0;a<2;++a){ogpu_next_image_copy copy={{OGPU_NEXT_ASPECT_COLOR,0,0,1,{0,0,0},{8,8,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};ogpu_next_copy_from_image(e,(ogpu_next_span){memory,1024+256*a,256},images[a],&copy);}
                ogpu_next_barrier_scope host={OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_HOST,OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_HOST_READ};dep=(ogpu_next_dependency){1,0,0,0,&host,NULL,NULL};ogpu_next_barrier(e,&dep,scratch);TRY(ogpu_next_commands_end(e,&list));
                for(uint32_t replay=0;replay<2;++replay){memset(map.data,0xa5,(size_t)md.size);
                    struct {float colors[2][4];uint32_t route;} control={{{replay?.5f:1,0,0,1},{0,replay?1:.5f,1,1}},0};
                    memcpy(map.data,&control,sizeof(control));control.route=1;memcpy((char*)map.data+64,&control,sizeof(control));unsigned char before[1024];memcpy(before,map.data,1024);
                    TRY(ogpu_next_memory_flush(all));ogpu_next_sync_point signal={{done,++serial},OGPU_NEXT_STAGE_ALL};ogpu_next_submit_desc submit={1,0,1,0,0,0,&list,NULL,&signal,NULL,NULL};ogpu_next_submit_outcome outcome={0};pending=1;
                    TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),1,&submit,&outcome,(ogpu_next_host_span){ss,sr.size}));TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000)));pending=0;TRY(ogpu_next_memory_invalidate(all));
                    unsigned char *out=(unsigned char*)map.data+1024;
                    if(!path)memcpy(reference[replay],out,512);else{if(memcmp(reference[replay],out,512))fprintf(stderr,"Viewport arrays mismatch: mode %u samples %u replay %u\n",mode,samples,replay);REQUIRE(!memcmp(reference[replay],out,512));}
                    for(uint32_t a=0;a<2;++a)for(uint32_t y=0;y<8;++y)for(uint32_t x=0;x<8;++x)for(uint32_t c=0;c<4;++c){uint32_t channel=a&&c!=3?2-c:c;float f=control.colors[x/4][channel];int expected=(int)(f*255+.5f);REQUIRE(abs((int)out[256*a+4*(8*y+x)+c]-expected)<=1);}
                    REQUIRE(!memcmp(before,map.data,1024));for(uint64_t i=1536;i<md.size;++i)REQUIRE(((unsigned char*)map.data)[i]==0xa5);pixels+=128;
                }
                TRY(ogpu_next_arena_reset(arena));
            }
            for(uint32_t i=0;i<3;++i){ogpu_next_executable_destroy(pipelines[i]);pipelines[i]=NULL;}
        }
        ++profiles;printf("Viewport arrays mode %u PASS: 1/4 samples, static/dynamic bytes, two shader-selected viewports/targets, changed-data replay, guards.\n",mode);
    }
    printf("Viewport arrays PASS: %u profiles, %" PRIu64 " submissions, %" PRIu64 " pixels, %u rejected recordings.\n",profiles,serial,pixels,rejected);result=EXIT_SUCCESS;
cleanup:
    if(pending){fputs("Pending viewport-array work after failure.\n",stderr);_Exit(EXIT_FAILURE);}
    ogpu_next_arena_destroy(arena);ogpu_next_timeline_destroy(done);for(uint32_t i=0;i<3;++i)ogpu_next_executable_destroy(pipelines[i]);ogpu_next_argument_layout_destroy(arguments);
    for(uint32_t i=0;i<4;++i){ogpu_next_view_destroy(views[i]);ogpu_next_image_destroy(images[i]);ogpu_next_memory_destroy(backing[i]);}for(uint32_t i=0;i<2;++i)free(code[i]);ogpu_next_memory_destroy(memory);
    if(file){fclose(file);}free(bs);free(rs);free(ss);return result;
}
