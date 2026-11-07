#include <math.h>
/* Native raster mechanisms: static/dynamic equivalence and coverage oracles. */
static int raster_mechanisms_execution(ogpu_next_device *device,ogpu_next_memory_desc md,
                                       uint32_t domain,const char *const paths[2]) {
    int result=EXIT_FAILURE,pending=0;FILE *file=NULL;
    void *code[2]={NULL},*scratch=NULL,*render_scratch=NULL,*submit_scratch=NULL;
    ogpu_next_memory *memory=NULL,*backing=NULL;ogpu_next_image *image=NULL;ogpu_next_view *view=NULL;
    ogpu_next_argument_layout *arguments=NULL;ogpu_next_executable *pipeline=NULL;
    ogpu_next_arena *arena=NULL;ogpu_next_timeline *done=NULL;
    ogpu_next_feature_info features={0};ogpu_next_query q=query(OGPU_NEXT_QUERY_FEATURES,&features,1,sizeof(features));TRY(ogpu_next_device_query(device,&q));
    ogpu_next_raster_mechanism_limits limits={0};q=query(OGPU_NEXT_QUERY_RASTER_MECHANISM_LIMITS,&limits,1,sizeof(limits));TRY(ogpu_next_device_query(device,&q));
    printf("Raster mechanism limits: over=%g extra=%g granularity=%g under=%u points-lines=%u degenerate-tri=%u degenerate-line=%u fully-covered=%u post-depth=%u provoking-per-pipeline=%u line-bits=%u strict=%u.\n",
        limits.primitive_overestimation,limits.max_extra_overestimation,limits.extra_overestimation_granularity,limits.underestimation,limits.conservative_points_lines,
        limits.degenerate_triangles,limits.degenerate_lines,limits.fully_covered_input,limits.conservative_post_depth_coverage,limits.provoking_per_pipeline,limits.line_subpixel_precision,limits.strict_lines);
    ogpu_next_argument_field field={0,8,OGPU_NEXT_ARGUMENT_INLINE,0,OGPU_NEXT_STAGE_VERTEX|OGPU_NEXT_STAGE_FRAGMENT,OGPU_NEXT_ARGUMENT_DIRECT,OGPU_NEXT_ARGUMENT_DIRECT};
    ogpu_next_argument_layout_desc ld={OGPU_NEXT_ARGUMENT_GRAPHICS,8,1,0,&field};TRY(ogpu_next_argument_layout_create(device,&ld,&arguments));
    const char *names[]={"vertexMain","fragmentMain"};
    const uint32_t shader_stages[]={OGPU_NEXT_STAGE_VERTEX,OGPU_NEXT_STAGE_FRAGMENT};ogpu_next_shader shaders[2];
    for(uint32_t i=0;i<2;++i) {
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
    ogpu_next_host_requirements br={0},rr={0},sr={0};TRY(ogpu_next_barrier_scratch_requirements(device,1,0,1,&br));TRY(ogpu_next_render_scratch_requirements(1,&rr));
    TRY(ogpu_next_submit_scratch_requirements(device,1,&(ogpu_next_submit_desc){.list_count=1,.signal_count=1},&sr));
    REQUIRE(br.alignment<=_Alignof(max_align_t) && rr.alignment<=_Alignof(max_align_t) && sr.alignment<=_Alignof(max_align_t));
    scratch=malloc((size_t)br.size);render_scratch=malloc((size_t)rr.size);submit_scratch=malloc((size_t)sr.size);REQUIRE(scratch && render_scratch && submit_scratch);
    ogpu_next_host_span bs={scratch,br.size};
    ogpu_next_arena_desc ad={HEADER(ogpu_next_arena_desc,OGPU_NEXT_ARENA_DESC),domain,1,OGPU_NEXT_PRIMARY,0};TRY(ogpu_next_arena_create(device,&ad,&arena));TRY(ogpu_next_timeline_create(device,0,&done));
    ogpu_next_recording_desc rd={HEADER(ogpu_next_recording_desc,OGPU_NEXT_RECORDING_DESC),OGPU_NEXT_SERIAL_REPLAY,OGPU_NEXT_PRIMARY,NULL};
    uint64_t serial=0,checked_pixels=0;uint32_t tested=0,rejected=0,valid[21]={0};
    unsigned char reference[21][2][256];
    for(uint32_t mode=0;mode<21;++mode) {
        uint32_t provoking=mode<3||mode==20,convention=mode>=3&&mode<=5,conservative=(mode>=6&&mode<=9)||(mode>=18&&mode<=19);
        uint32_t line=mode>=10&&mode<=16,clip=mode==17;
        uint32_t line_mode=mode==16?2:line?1+(mode-10)%3:0,stipple=line&&mode>=13;
        uint32_t cmode=mode==6?0:mode==8?2:1;
        uint64_t dynamic=mode==2?OGPU_NEXT_DYNAMIC_PROVOKING_VERTEX:mode==5?OGPU_NEXT_DYNAMIC_DEPTH_CONVENTION:
            mode==9?(OGPU_NEXT_DYNAMIC_CONSERVATIVE_MODE|OGPU_NEXT_DYNAMIC_EXTRA_OVERESTIMATION):
            mode==16?(OGPU_NEXT_DYNAMIC_LINE_MODE|OGPU_NEXT_DYNAMIC_LINE_STIPPLE_ENABLE|OGPU_NEXT_DYNAMIC_LINE_STIPPLE):
            clip?OGPU_NEXT_DYNAMIC_DEPTH_CLIP:mode==18?OGPU_NEXT_DYNAMIC_CONSERVATIVE_MODE:mode==19?OGPU_NEXT_DYNAMIC_EXTRA_OVERESTIMATION:0;
        uint32_t needed[5]={0},n=0;
        if(provoking)needed[n++]=OGPU_NEXT_FEATURE_PROVOKING_LAST;
        if(mode==20){needed[n++]=OGPU_NEXT_FEATURE_CONSERVATIVE_RASTER;needed[n++]=OGPU_NEXT_FEATURE_DEPTH_NEGATIVE_ONE_TO_ONE;needed[n++]=OGPU_NEXT_FEATURE_DEPTH_CLIP;needed[n++]=OGPU_NEXT_FEATURE_BRESENHAM_LINES;}
        if(convention)needed[n++]=OGPU_NEXT_FEATURE_DEPTH_NEGATIVE_ONE_TO_ONE;
        if(conservative)needed[n++]=OGPU_NEXT_FEATURE_CONSERVATIVE_RASTER;
        if(line)needed[n++]=OGPU_NEXT_FEATURE_RECTANGULAR_LINES+line_mode-1;
        if(stipple)needed[n++]=OGPU_NEXT_FEATURE_STIPPLED_RECTANGULAR_LINES+line_mode-1;
        if(mode==2)needed[n++]=OGPU_NEXT_FEATURE_DYNAMIC_PROVOKING_VERTEX;
        if(mode==5)needed[n++]=OGPU_NEXT_FEATURE_DYNAMIC_DEPTH_CONVENTION;
        if(mode==9){needed[n++]=OGPU_NEXT_FEATURE_DYNAMIC_CONSERVATIVE_MODE;needed[n++]=OGPU_NEXT_FEATURE_DYNAMIC_EXTRA_OVERESTIMATION;}
        if(mode==18)needed[n++]=OGPU_NEXT_FEATURE_DYNAMIC_CONSERVATIVE_MODE;
        if(mode==19)needed[n++]=OGPU_NEXT_FEATURE_DYNAMIC_EXTRA_OVERESTIMATION;
        if(mode==16){needed[n++]=OGPU_NEXT_FEATURE_DYNAMIC_LINE_MODE;needed[n++]=OGPU_NEXT_FEATURE_DYNAMIC_LINE_STIPPLE_ENABLE;}
        if(clip){needed[n++]=OGPU_NEXT_FEATURE_DEPTH_CLIP;needed[n++]=OGPU_NEXT_FEATURE_DYNAMIC_DEPTH_CLIP;}
        uint32_t supported=!(mode==8&&!limits.underestimation);
        for(uint32_t i=0;i<n;++i)if(!raster_has(features.enabled,needed[i]))supported=0;
        if(!supported){printf("Raster mechanism mode %u UNVERIFIED: unsupported native profile.\n",mode);continue;}
        ogpu_next_shader_requirements requirements={HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),features.enabled,{0},0};
        ogpu_next_color_state color={.format=OGPU_NEXT_RGBA8_UNORM,.write_mask=15};
        ogpu_next_provoking_state ps={HEADER(ogpu_next_provoking_state,OGPU_NEXT_PROVOKING_STATE),mode?1:0,0};
        ogpu_next_depth_convention_state ds={HEADER(ogpu_next_depth_convention_state,OGPU_NEXT_DEPTH_CONVENTION_STATE),mode==3?0:1,0};
        ogpu_next_conservative_state cs={HEADER(ogpu_next_conservative_state,OGPU_NEXT_CONSERVATIVE_STATE),cmode,0};
        ogpu_next_line_state ls={HEADER(ogpu_next_line_state,OGPU_NEXT_LINE_STATE),line_mode,stipple,1,0x5555};
        ogpu_next_depth_clip_state dc={HEADER(ogpu_next_depth_clip_state,OGPU_NEXT_DEPTH_CLIP_STATE),0,0};
        if(mode==2)ps.mode=UINT32_MAX;
        if(mode==5)ds.negative_one_to_one=UINT32_MAX;
        if(mode==9||mode==18)cs.mode=UINT32_MAX;
        if(mode==9||mode==19)cs.extra_overestimation=NAN;
        if(mode==16){ls.mode=UINT32_MAX;ls.stipple_enable=UINT32_MAX;ls.factor=0;ls.pattern=UINT32_MAX;}
        if(clip)dc.enable=UINT32_MAX;
        ogpu_next_raster_options options={.header=HEADER(ogpu_next_raster_options,OGPU_NEXT_RASTER_OPTIONS),.line_width=1,.max_depth_bounds=1,.sample_mask=UINT64_MAX};
        options.header.next=provoking?&ps.header:convention?&ds.header:conservative?&cs.header:line?&ls.header:&dc.header;
        if(mode==20){cs.mode=0;ls.mode=2;ds.negative_one_to_one=1;dc.enable=1;cs.header.next=&ls.header;ls.header.next=&ps.header;ps.header.next=&ds.header;ds.header.next=&dc.header;options.header.next=&cs.header;}
        ogpu_next_graphics_state gs={.header=HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),.topology=line?OGPU_NEXT_LINES:OGPU_NEXT_TRIANGLES,
            .front_face=OGPU_NEXT_FRONT_CCW,.samples=1,.color_count=1,.colors=&color,.raster_options=&options.header};
        ogpu_next_executable_desc ed={HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_GRAPHICS,2,shaders,&gs.header,
            OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR|dynamic,&requirements.header,NULL,0,0};
        if(mode==0||mode==3||mode==6||mode==10) {
            for(uint32_t bad=0;bad<3;++bad) {
                ogpu_next_record *h=provoking?&ps.header:convention?&ds.header:conservative?&cs.header:&ls.header;
                if(bad==0){if(provoking)ps.mode=2;else if(convention)ds.negative_one_to_one=2;else if(conservative)cs.mode=3;else ls.mode=4;}
                if(bad==1)h->next=h;
                if(bad==2){if(provoking)ps.reserved=1;else if(convention)ds.reserved=1;else if(conservative)cs.extra_overestimation=NAN;else ls.factor=257;}
                ogpu_next_executable *invalid=NULL;REQUIRE(ogpu_next_executable_create(device,&ed,arguments,&invalid)!=OGPU_NEXT_OK);REQUIRE(!invalid);++rejected;
                h->next=NULL;ps.mode=0;ps.reserved=0;ds.negative_one_to_one=0;ds.reserved=0;cs.mode=0;cs.extra_overestimation=0;ls.mode=line_mode;ls.factor=1;
            }
        }
        TRY(ogpu_next_executable_create(device,&ed,arguments,&pipeline));
        ogpu_next_encoder *e=NULL;ogpu_next_list *list=NULL;TRY(ogpu_next_commands_begin(arena,&rd,&e));
        ogpu_next_image_barrier ib={{0,OGPU_NEXT_STAGE_COLOR,0,OGPU_NEXT_ACCESS_COLOR_WRITE},image,range,OGPU_NEXT_STATE_UNDEFINED,OGPU_NEXT_STATE_COLOR_ATTACHMENT,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,1,0};
        ogpu_next_dependency dep={0,0,1,0,NULL,NULL,&ib};ogpu_next_barrier(e,&dep,bs);
        ogpu_next_attachment a={view,NULL,OGPU_NEXT_STATE_COLOR_ATTACHMENT,0,OGPU_NEXT_CLEAR,OGPU_NEXT_STORE,0,{.f32={0,0,0,1}}};
        ogpu_next_render_desc render={.header=HEADER(ogpu_next_render_desc,OGPU_NEXT_RENDER_DESC),.width=8,.height=8,.layers=1,.samples=1,.color_count=1,.colors=&a,.scratch={render_scratch,rr.size}};
        ogpu_next_viewport_state vp={HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),0,0,8,8,0,1,0,0,8,8};
        ogpu_next_render_begin(e,&render);ogpu_next_bind_executable(e,pipeline);ogpu_next_set_graphics_state(e,&vp.header);ogpu_next_set_arguments(e,arguments,0,8,&address);
        if(mode==2){ogpu_next_set_provoking_vertex(e,0);ogpu_next_set_provoking_vertex(e,1);}
        if(mode==5){ogpu_next_set_depth_convention(e,0);ogpu_next_set_depth_convention(e,1);}
        if(mode==9||mode==18){ogpu_next_set_conservative_mode(e,0);ogpu_next_set_conservative_mode(e,1);}
        if(mode==9||mode==19)ogpu_next_set_extra_overestimation(e,0);
        if(mode==16){ogpu_next_set_line_mode(e,0);ogpu_next_set_line_mode(e,2);ogpu_next_set_line_stipple_enable(e,0);ogpu_next_set_line_stipple_enable(e,1);ogpu_next_set_line_stipple(e,1,0);ogpu_next_set_line_stipple(e,1,0x5555);}
        if(clip){ogpu_next_set_depth_clip(e,1);ogpu_next_set_depth_clip(e,0);}
        ogpu_next_draw_desc draw={line?2u:3u,1,0,0,0};ogpu_next_draw(e,&draw);ogpu_next_render_end(e);
        ib=(ogpu_next_image_barrier){{OGPU_NEXT_STAGE_COLOR,OGPU_NEXT_STAGE_COPY,OGPU_NEXT_ACCESS_COLOR_WRITE,OGPU_NEXT_ACCESS_COPY_READ},image,range,OGPU_NEXT_STATE_COLOR_ATTACHMENT,OGPU_NEXT_STATE_COPY_SRC,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,0,0};ogpu_next_barrier(e,&dep,bs);
        ogpu_next_image_copy copy={{OGPU_NEXT_ASPECT_COLOR,0,0,1,{0,0,0},{8,8,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};ogpu_next_copy_from_image(e,(ogpu_next_span){memory,1024,256},image,&copy);
        ogpu_next_barrier_scope host={OGPU_NEXT_STAGE_COPY,OGPU_NEXT_STAGE_HOST,OGPU_NEXT_ACCESS_COPY_WRITE,OGPU_NEXT_ACCESS_HOST_READ};dep=(ogpu_next_dependency){1,0,0,0,&host,NULL,NULL};ogpu_next_barrier(e,&dep,bs);
        TRY(ogpu_next_commands_end(e,&list));
        for(uint32_t replay=0;replay<2;++replay) {
            memset(map.data,0xa5,(size_t)md.size);
            struct {float depth,size;uint32_t unused,shape;} control={(convention||clip||mode==20)?(replay?.5f:-.5f):0,4.0f+2*replay,0,provoking?0:conservative?(replay?4:1):line?2:3};
            memcpy(map.data,&control,sizeof(control));unsigned char before[1024];memcpy(before,map.data,sizeof(before));
            TRY(ogpu_next_memory_flush(all));ogpu_next_sync_point signal={{done,++serial},OGPU_NEXT_STAGE_ALL};ogpu_next_submit_desc submit={1,0,1,0,0,0,&list,NULL,&signal,NULL,NULL};ogpu_next_submit_outcome outcome={0};pending=1;
            TRY(ogpu_next_queue_submit(ogpu_next_device_queue(device,domain,0),1,&submit,&outcome,(ogpu_next_host_span){submit_scratch,sr.size}));TRY(ogpu_next_timeline_wait(signal.point,UINT64_C(10000000000)));pending=0;TRY(ogpu_next_memory_invalidate(all));
            unsigned char *pixels=(unsigned char*)map.data+1024;uint32_t covered=0;
            for(uint32_t y=0;y<8;++y)for(uint32_t x=0;x<8;++x) {
                unsigned char *pixel=pixels+4*(y*8+x);uint32_t visible=pixel[0]!=0||pixel[1]!=0||pixel[2]!=0;covered+=visible;
                if(provoking||convention||clip){uint32_t on=!(mode==3&&!replay);unsigned char expected[4]={on?(provoking?(mode==0?17:79):255):0,on?(provoking?4+2*replay:255):0,on?(provoking?37:255):0,255};
                    if(memcmp(pixel,expected,4)){fprintf(stderr,"Raster mechanism %u replay %u pixel %u,%u: %u,%u,%u,%u expected %u,%u,%u,%u\n",mode,replay,x,y,pixel[0],pixel[1],pixel[2],pixel[3],expected[0],expected[1],expected[2],expected[3]);}REQUIRE(!memcmp(pixel,expected,4));}
                else if(conservative){REQUIRE(pixel[3]==255);for(uint32_t c=0;c<3;++c)REQUIRE(pixel[c]==(visible?255:0));if(visible)REQUIRE(x>=3&&x<=6&&y>=3&&y<=6);}
                else {REQUIRE(pixel[0]==pixel[1]&&pixel[1]==pixel[2]);if(visible)REQUIRE(x>=1&&x<=6&&y>=3&&y<=5);if(line_mode!=3){REQUIRE(pixel[3]==255);REQUIRE(pixel[0]==(visible?255:0));if(visible)REQUIRE(y==4);}}
                ++checked_pixels;
            }
            if(conservative){if(mode==6)REQUIRE(covered==replay);else if(mode==8)REQUIRE(covered==0);else {REQUIRE(covered>0);REQUIRE(pixels[4*(4*8+4)]==255);}}
            if(line){if(line_mode==3)REQUIRE(covered>0);else REQUIRE(covered==(stipple?3u:6u));}
            memcpy(reference[mode][replay],pixels,256);
            uint32_t previous=mode==2?1:mode==5?4:(mode==9||mode==18||mode==19)?7:mode==16?14:UINT32_MAX;
            if(previous!=UINT32_MAX&&valid[previous])REQUIRE(!memcmp(reference[previous][replay],pixels,256));
            REQUIRE(!memcmp(map.data,before,sizeof(before)));for(uint64_t i=1280;i<md.size;++i)REQUIRE(((unsigned char*)map.data)[i]==0xa5);
            printf("Raster mechanism mode %u replay %u coverage=%u.\n",mode,replay,covered);
        }
        valid[mode]=1;++tested;TRY(ogpu_next_arena_reset(arena));ogpu_next_executable_destroy(pipeline);pipeline=NULL;
    }
    printf("Raster mechanisms PASS: %u profiles, %" PRIu64 " submissions, %" PRIu64 " pixels, %u rejected preparations and guards.\n",tested,serial,checked_pixels,rejected);result=EXIT_SUCCESS;
cleanup:
    if(pending){fputs("Pending raster-mechanism work after failure.\n",stderr);_Exit(EXIT_FAILURE);}
    ogpu_next_arena_destroy(arena);ogpu_next_timeline_destroy(done);ogpu_next_executable_destroy(pipeline);ogpu_next_argument_layout_destroy(arguments);
    ogpu_next_view_destroy(view);ogpu_next_image_destroy(image);ogpu_next_memory_destroy(backing);ogpu_next_memory_destroy(memory);
    if(file){fclose(file);}for(uint32_t i=0;i<2;++i){free(code[i]);}free(scratch);free(render_scratch);free(submit_scratch);return result;
}
