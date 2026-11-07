#include "advanced_local_ms.h"
/* Consumer-owned attachment setup and one native local-read pass. */
static int advanced_local_setup(Textured *t) {
    Scene *s=&t->scene;
    ogpu_next_local_read_limits limits;ogpu_next_query q=query(OGPU_NEXT_QUERY_LOCAL_READ_LIMITS,&limits,1,sizeof(limits));
    GPU(ogpu_next_device_query(s->device,&q));NEED(limits.depth_stencil && limits.max_inputs>=2);
    ogpu_next_image_desc id={HEADER(ogpu_next_image_desc,OGPU_NEXT_IMAGE_DESC),OGPU_NEXT_RGBA8_UNORM,OGPU_NEXT_IMAGE_2D,1,1,1,
        {s->width,s->height,1},OGPU_NEXT_IMAGE_COLOR_ATTACHMENT|OGPU_NEXT_IMAGE_COPY_SRC,0,NULL,0,0,NULL};
    uint32_t types[32],type;ogpu_next_requirements req={.compatible_types=types,.compatible_type_capacity=32};
    GPU(ogpu_next_image_requirements(s->device,&id,&req));NEED(memory_type(s,&req,OGPU_NEXT_MEMORY_LOCAL,&type));
    GPU(ogpu_next_image_create_unbound(s->device,&id,&t->local_image));
    GPU(ogpu_next_memory_create_dedicated_image(t->local_image,type,&t->local_backing));
    GPU(ogpu_next_image_bind(t->local_image,(ogpu_next_span){t->local_backing,0,req.size}));
    ogpu_next_view_desc vd={HEADER(ogpu_next_view_desc,OGPU_NEXT_VIEW_DESC),OGPU_NEXT_RGBA8_UNORM,OGPU_NEXT_VIEW_2D,OGPU_NEXT_IMAGE_COLOR_ATTACHMENT,{0},{OGPU_NEXT_ASPECT_COLOR,0,1,0,1}};
    GPU(ogpu_next_view_create(t->local_image,&vd,&t->local_view));
    ogpu_next_memory_desc md={HEADER(ogpu_next_memory_desc,OGPU_NEXT_MEMORY_DESC),s->pixels+2*GUARD,4,OGPU_NEXT_USAGE_COPY_DST,0,OGPU_NEXT_MEMORY_LINEAR,NULL,0,0};
    GPU(ogpu_next_memory_requirements(s->device,&md,&req));NEED(memory_type(s,&req,OGPU_NEXT_MEMORY_HOST_VISIBLE,&md.memory_type));
    GPU(ogpu_next_memory_create(s->device,&md,&t->local_readback));
    GPU(ogpu_next_memory_map((ogpu_next_span){t->local_readback,0,md.size},&t->local_map));
    memset(t->local_map.data,0xa5,md.size);GPU(ogpu_next_memory_flush((ogpu_next_span){t->local_readback,0,md.size}));
    for(unsigned i=0;i<2;++i) {
        ogpu_next_resource_descriptor input={OGPU_NEXT_DESCRIPTOR_INPUT_ATTACHMENT,OGPU_NEXT_STATE_LOCAL_READ,s->views[i],{0}};
        ogpu_next_heap_location location={t->bindings[0],t->local_stride*(i+1)};
        GPU(ogpu_next_write_resource_descriptors(s->device,1,&input,&location,t->scratch));
    }
    GPU(ogpu_next_memory_flush((ogpu_next_span){t->heaps[0],t->local_stride,2*t->local_stride}));
    ogpu_next_argument_field field={0,8,OGPU_NEXT_ARGUMENT_INLINE,0,OGPU_NEXT_STAGE_VERTEX|OGPU_NEXT_STAGE_FRAGMENT,OGPU_NEXT_ARGUMENT_DIRECT,OGPU_NEXT_ARGUMENT_DIRECT};
    ogpu_next_argument_layout_desc ld={OGPU_NEXT_ARGUMENT_GRAPHICS,8,1,0,&field};
    GPU(ogpu_next_argument_layout_create(s->device,&ld,&t->local_arguments));
    uint32_t locations_array[2]={OGPU_NEXT_ATTACHMENT_UNUSED,0},inputs_array[2]={0,OGPU_NEXT_ATTACHMENT_UNUSED},depth=1,unused=OGPU_NEXT_ATTACHMENT_UNUSED;
    ogpu_next_render_locations locations={2,0,locations_array};ogpu_next_render_inputs inputs={2,0,inputs_array,&depth,&unused};
    ogpu_next_input_attachment_binding bindings[2]={{0,0,0,(uint32_t)t->local_stride},{0,1,4,(uint32_t)t->local_stride}};
    ogpu_next_render_mapping mapping={HEADER(ogpu_next_render_mapping,OGPU_NEXT_RENDER_MAPPING),&locations,&inputs,2,0,bindings};
    ogpu_next_color_state colors[2]={{.format=OGPU_NEXT_RGBA8_UNORM,.write_mask=15},{.format=OGPU_NEXT_RGBA8_UNORM,.write_mask=15}};
    ogpu_next_graphics_state gs={.header=HEADER(ogpu_next_graphics_state,OGPU_NEXT_GRAPHICS_STATE),.topology=OGPU_NEXT_TRIANGLES,.front_face=OGPU_NEXT_FRONT_CCW,
        .samples=1,.color_count=2,.colors=colors,.depth_format=OGPU_NEXT_D32_FLOAT};gs.header.next=&mapping.header;
    ogpu_next_shader shaders[2]={advanced_local_vertex_shader(),advanced_local_fragment_shader()};
    uint32_t features[]={OGPU_NEXT_FEATURE_RASTER,OGPU_NEXT_FEATURE_LOCAL_READ};
    ogpu_next_shader_requirements requirements={HEADER(ogpu_next_shader_requirements,OGPU_NEXT_SHADER_REQUIREMENTS),{features,2,0},{0},0};
    ogpu_next_executable_desc ed={HEADER(ogpu_next_executable_desc,OGPU_NEXT_EXECUTABLE_DESC),OGPU_NEXT_EXECUTABLE_GRAPHICS,2,shaders,&gs.header,
        OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR,&requirements.header,NULL,0,0};
    GPU(ogpu_next_executable_create(s->device,&ed,t->local_arguments,&t->local_pipeline));
    NEED(advanced_local_ms(t));
    /* Restore source descriptors explicitly after the independent 4-sample probe. */
    for(unsigned i=0;i<2;++i) {
        ogpu_next_resource_descriptor input={OGPU_NEXT_DESCRIPTOR_INPUT_ATTACHMENT,OGPU_NEXT_STATE_LOCAL_READ,s->views[i],{0}};
        ogpu_next_heap_location location={t->bindings[0],t->local_stride*(i+1)};
        GPU(ogpu_next_write_resource_descriptors(s->device,1,&input,&location,t->scratch));
    }
    GPU(ogpu_next_memory_flush((ogpu_next_span){t->heaps[0],t->local_stride,2*t->local_stride}));return 1;
}
static void advanced_local_draw(Textured *t) {
    Scene *s=&t->scene;
    ogpu_next_barrier_scope scope={OGPU_NEXT_STAGE_COLOR|OGPU_NEXT_STAGE_DEPTH,OGPU_NEXT_STAGE_FRAGMENT,
        OGPU_NEXT_ACCESS_COLOR_WRITE|OGPU_NEXT_ACCESS_DEPTH_WRITE,OGPU_NEXT_ACCESS_INPUT_ATTACHMENT_READ};
    ogpu_next_dependency dep={1,0,0,OGPU_NEXT_DEPENDENCY_BY_REGION,&scope,NULL,NULL};
    ogpu_next_barrier(s->encoder,&dep,(ogpu_next_host_span){s->bs,s->barrier_scratch.size});
    uint32_t values[2]={OGPU_NEXT_ATTACHMENT_UNUSED,0};ogpu_next_render_locations locations={2,0,values};
    ogpu_next_set_render_locations(s->encoder,&locations);ogpu_next_bind_executable(s->encoder,t->local_pipeline);
    ogpu_next_viewport_state viewport={HEADER(ogpu_next_viewport_state,OGPU_NEXT_VIEWPORT_STATE),0,0,(float)s->width,(float)s->height,0,1,0,0,s->width,s->height};
    ogpu_next_set_graphics_state(s->encoder,&viewport.header);
    uint32_t root[2]={(uint32_t)t->local_stride,(uint32_t)(2*t->local_stride)};
    ogpu_next_set_arguments(s->encoder,t->local_arguments,0,sizeof(root),root);
    ogpu_next_draw_desc draw={3,1,0,0,0};ogpu_next_draw(s->encoder,&draw);
}
static void advanced_local_transition(Textured *t,int discard) {
    Scene *s=&t->scene;
    ogpu_next_image_barrier b={{discard?OGPU_NEXT_STAGE_COPY:OGPU_NEXT_STAGE_COLOR,discard?OGPU_NEXT_STAGE_COLOR:OGPU_NEXT_STAGE_COPY,
        discard?OGPU_NEXT_ACCESS_COPY_READ:OGPU_NEXT_ACCESS_COLOR_WRITE,discard?OGPU_NEXT_ACCESS_COLOR_WRITE:OGPU_NEXT_ACCESS_COPY_READ},
        t->local_image,{OGPU_NEXT_ASPECT_COLOR,0,1,0,1},discard?OGPU_NEXT_STATE_UNDEFINED:OGPU_NEXT_STATE_GENERAL,
        discard?OGPU_NEXT_STATE_GENERAL:OGPU_NEXT_STATE_COPY_SRC,OGPU_NEXT_DOMAIN_IGNORED,OGPU_NEXT_DOMAIN_IGNORED,(uint32_t)discard,0,NULL};
    ogpu_next_dependency dep={0,0,1,0,NULL,NULL,&b};ogpu_next_barrier(s->encoder,&dep,(ogpu_next_host_span){s->bs,s->barrier_scratch.size});
}
static void advanced_local_copy(Textured *t) {
    Scene *s=&t->scene;advanced_local_transition(t,0);
    ogpu_next_image_copy copy={{OGPU_NEXT_ASPECT_COLOR,0,0,1,{0,0,0},{s->width,s->height,1}},0,0,OGPU_NEXT_STATE_COPY_SRC,0};
    ogpu_next_copy_from_image(s->encoder,(ogpu_next_span){t->local_readback,GUARD,s->pixels},t->local_image,&copy);
}
static void advanced_local_destroy(Textured *t) {
    ogpu_next_executable_destroy(t->local_pipeline);ogpu_next_argument_layout_destroy(t->local_arguments);
    ogpu_next_view_destroy(t->local_view);ogpu_next_image_destroy(t->local_image);ogpu_next_memory_destroy(t->local_backing);ogpu_next_memory_destroy(t->local_readback);
}
