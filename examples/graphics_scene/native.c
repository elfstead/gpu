/* Direct native indexed/depth reference. No OGPU execution calls or linkage. */
#define main learned_image_unused_main
#include "../learned_image/native.c"
#undef main

enum { GUARD = 64, MODES = 10 };
typedef struct { uint64_t vertices, indices, draws; uint32_t phase, empty; } SceneRoot;
_Static_assert(sizeof(SceneRoot) == 32 && offsetof(SceneRoot, phase) == 24, "native root layout");
_Static_assert(sizeof(VkDrawIndexedIndirectCommand) == 20, "native indirect layout");
typedef struct {
    Native n;
    NativeBatch batch;
    NativeProgram prepare, raster[4];
    NativeImage color, depth;
    NativeBuffer vertices, indices, draws, upload, readback, geometry;
    PFN_vkCmdBindIndexBuffer3KHR bind_index;
    PFN_vkCmdDrawIndexedIndirect2KHR draw_indexed;
    uint32_t width, height, index_bytes;
    size_t pixels;
    unsigned char *cpu;
} Scene;

static int scene_shader(Native *n, NativeProgram *p, unsigned stage, const char *directory, const char *name) {
    char path[4096]; NEED(native_path(path, directory, name));
    FILE *f = fopen(path, "rb"); if (!f) { perror(path); return 0; }
    int okay = 0; long size = 0; uint32_t *code = NULL;
    if (fseek(f, 0, SEEK_END) || (size = ftell(f)) < 20 || size > 1024*1024 || size%4 || fseek(f, 0, SEEK_SET)) goto done;
    code = malloc((size_t)size); if (!code || fread(code, 1, (size_t)size, f) != (size_t)size || ferror(f)) goto done;
    if (stage == 2) okay = native_compute(n, p, code, (size_t)size, sizeof(SceneRoot), (uint32_t[]){8,1,1});
    else okay = native_module(n, code, (size_t)size, &p->modules[stage]);
done:
    free(code); if (fclose(f)) okay = 0; return okay;
}

static int scene_raster(Scene *s, unsigned mode, const char *directory) {
    Native *n = &s->n; NativeProgram *p = &s->raster[mode]; p->root_size = 8;
    NEED(scene_shader(n, p, 0, directory, "vertex.spv") && scene_shader(n, p, 1, directory, "fragment.spv"));
    VkPipelineShaderStageCreateInfo stages[2] = {
        {.sType=VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO, .stage=VK_SHADER_STAGE_VERTEX_BIT, .module=p->modules[0], .pName="main"},
        {.sType=VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO, .stage=VK_SHADER_STAGE_FRAGMENT_BIT, .module=p->modules[1], .pName="main"}};
    VkPipelineVertexInputStateCreateInfo vertex = {.sType=VK_STRUCTURE_TYPE_PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO};
    VkPipelineInputAssemblyStateCreateInfo assembly = {.sType=VK_STRUCTURE_TYPE_PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO,
        .topology=VK_PRIMITIVE_TOPOLOGY_TRIANGLE_LIST};
    VkPipelineViewportStateCreateInfo viewport = {.sType=VK_STRUCTURE_TYPE_PIPELINE_VIEWPORT_STATE_CREATE_INFO, .viewportCount=1, .scissorCount=1};
    VkPipelineRasterizationStateCreateInfo raster = {.sType=VK_STRUCTURE_TYPE_PIPELINE_RASTERIZATION_STATE_CREATE_INFO,
        .polygonMode=VK_POLYGON_MODE_FILL, .frontFace=VK_FRONT_FACE_COUNTER_CLOCKWISE, .lineWidth=1};
    VkPipelineMultisampleStateCreateInfo samples = {.sType=VK_STRUCTURE_TYPE_PIPELINE_MULTISAMPLE_STATE_CREATE_INFO, .rasterizationSamples=VK_SAMPLE_COUNT_1_BIT};
    VkPipelineDepthStencilStateCreateInfo depth = {.sType=VK_STRUCTURE_TYPE_PIPELINE_DEPTH_STENCIL_STATE_CREATE_INFO,
        .depthTestEnable=mode != 1, .depthWriteEnable=mode != 2, .depthCompareOp=mode == 3 ? VK_COMPARE_OP_ALWAYS : VK_COMPARE_OP_LESS};
    VkPipelineColorBlendAttachmentState attachment = {.colorWriteMask=15};
    VkPipelineColorBlendStateCreateInfo blend = {.sType=VK_STRUCTURE_TYPE_PIPELINE_COLOR_BLEND_STATE_CREATE_INFO, .attachmentCount=1, .pAttachments=&attachment};
    VkDynamicState states[] = {VK_DYNAMIC_STATE_VIEWPORT, VK_DYNAMIC_STATE_SCISSOR};
    VkPipelineDynamicStateCreateInfo dynamic = {.sType=VK_STRUCTURE_TYPE_PIPELINE_DYNAMIC_STATE_CREATE_INFO, .dynamicStateCount=2, .pDynamicStates=states};
    VkPipelineCreateFlags2CreateInfo flags = {.sType=VK_STRUCTURE_TYPE_PIPELINE_CREATE_FLAGS_2_CREATE_INFO, .flags=VK_PIPELINE_CREATE_2_DESCRIPTOR_HEAP_BIT_EXT};
    VkFormat format = VK_FORMAT_R8G8B8A8_UNORM;
    VkPipelineRenderingCreateInfo rendering = {.sType=VK_STRUCTURE_TYPE_PIPELINE_RENDERING_CREATE_INFO, .pNext=&flags,
        .colorAttachmentCount=1, .pColorAttachmentFormats=&format, .depthAttachmentFormat=VK_FORMAT_D32_SFLOAT};
    VkGraphicsPipelineCreateInfo create = {.sType=VK_STRUCTURE_TYPE_GRAPHICS_PIPELINE_CREATE_INFO, .pNext=&rendering,
        .stageCount=2, .pStages=stages, .pVertexInputState=&vertex, .pInputAssemblyState=&assembly, .pViewportState=&viewport,
        .pRasterizationState=&raster, .pMultisampleState=&samples, .pDepthStencilState=&depth, .pColorBlendState=&blend,
        .pDynamicState=&dynamic, .basePipelineIndex=-1};
    VK_TRY(n->vkCreateGraphicsPipelines(n->device, VK_NULL_HANDLE, 1, &create, NULL, &p->pipeline)); return 1;
}

static int scene_depth(Scene *s) {
    Native *n = &s->n; NativeImage *im = &s->depth;
    VkImageUsageFlags usage = VK_IMAGE_USAGE_DEPTH_STENCIL_ATTACHMENT_BIT | VK_IMAGE_USAGE_TRANSFER_SRC_BIT;
    VkImageFormatProperties support;
    VK_TRY(n->vkGetPhysicalDeviceImageFormatProperties(n->physical, VK_FORMAT_D32_SFLOAT, VK_IMAGE_TYPE_2D,
        VK_IMAGE_TILING_OPTIMAL, usage, 0, &support));
    NEED(s->width <= support.maxExtent.width && s->height <= support.maxExtent.height && support.maxExtent.depth >= 1
        && support.maxMipLevels && support.maxArrayLayers && (support.sampleCounts & VK_SAMPLE_COUNT_1_BIT)
        && s->pixels <= support.maxResourceSize);
    im->width=s->width; im->height=s->height;
    VkImageCreateInfo create = {.sType=VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO, .imageType=VK_IMAGE_TYPE_2D,
        .format=VK_FORMAT_D32_SFLOAT, .extent={s->width,s->height,1}, .mipLevels=1, .arrayLayers=1,
        .samples=VK_SAMPLE_COUNT_1_BIT, .tiling=VK_IMAGE_TILING_OPTIMAL, .usage=usage,
        .sharingMode=VK_SHARING_MODE_EXCLUSIVE, .initialLayout=VK_IMAGE_LAYOUT_UNDEFINED};
    VK_TRY(n->vkCreateImage(n->device, &create, NULL, &im->image));
    VkMemoryRequirements req; n->vkGetImageMemoryRequirements(n->device, im->image, &req);
    NEED(native_image_type(&n->memory, req.memoryTypeBits, &im->type)); im->allocated=req.size;
    VkMemoryDedicatedAllocateInfo dedicated = {.sType=VK_STRUCTURE_TYPE_MEMORY_DEDICATED_ALLOCATE_INFO, .image=im->image};
    VkMemoryAllocateInfo allocate = {.sType=VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO, .pNext=&dedicated, .allocationSize=req.size, .memoryTypeIndex=im->type};
    VK_TRY(n->vkAllocateMemory(n->device, &allocate, NULL, &im->memory));
    VK_TRY(n->vkBindImageMemory(n->device, im->image, im->memory, 0));
    VkImageViewCreateInfo view = {.sType=VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO, .image=im->image,
        .viewType=VK_IMAGE_VIEW_TYPE_2D, .format=VK_FORMAT_D32_SFLOAT,
        .subresourceRange={.aspectMask=VK_IMAGE_ASPECT_DEPTH_BIT, .levelCount=1, .layerCount=1}};
    VK_TRY(n->vkCreateImageView(n->device, &view, NULL, &im->view)); return 1;
}

static void scene_transition(Scene *s) {
    VkImageMemoryBarrier2 barriers[2] = {0};
    for (unsigned i=0; i<2; ++i) barriers[i] = (VkImageMemoryBarrier2){.sType=VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER_2,
        .srcStageMask=VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT, .srcAccessMask=VK_ACCESS_2_MEMORY_READ_BIT | VK_ACCESS_2_MEMORY_WRITE_BIT,
        .dstStageMask=VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT, .dstAccessMask=VK_ACCESS_2_MEMORY_READ_BIT | VK_ACCESS_2_MEMORY_WRITE_BIT,
        .oldLayout=VK_IMAGE_LAYOUT_UNDEFINED, .newLayout=VK_IMAGE_LAYOUT_GENERAL,
        .srcQueueFamilyIndex=VK_QUEUE_FAMILY_IGNORED, .dstQueueFamilyIndex=VK_QUEUE_FAMILY_IGNORED,
        .image=i ? s->depth.image : s->color.image,
        .subresourceRange={.aspectMask=i ? VK_IMAGE_ASPECT_DEPTH_BIT : VK_IMAGE_ASPECT_COLOR_BIT, .levelCount=1, .layerCount=1}};
    VkDependencyInfo dep = {.sType=VK_STRUCTURE_TYPE_DEPENDENCY_INFO, .imageMemoryBarrierCount=2, .pImageMemoryBarriers=barriers};
    s->n.vkCmdPipelineBarrier2(s->batch.command, &dep);
}

static int scene_pass(Scene *s, unsigned pipeline, int color_load, int depth_load, float clear,
                      unsigned first, unsigned count) {
    Native *n=&s->n; NativeBatch *b=&s->batch; NativeProgram *p=&s->raster[pipeline];
    VkRenderingAttachmentInfo color = {.sType=VK_STRUCTURE_TYPE_RENDERING_ATTACHMENT_INFO, .imageView=s->color.view,
        .imageLayout=VK_IMAGE_LAYOUT_GENERAL, .loadOp=color_load ? VK_ATTACHMENT_LOAD_OP_LOAD : VK_ATTACHMENT_LOAD_OP_CLEAR,
        .storeOp=VK_ATTACHMENT_STORE_OP_STORE, .clearValue={.color={.float32={0,0,0,1}}}};
    VkRenderingAttachmentInfo depth = {.sType=VK_STRUCTURE_TYPE_RENDERING_ATTACHMENT_INFO, .imageView=s->depth.view,
        .imageLayout=VK_IMAGE_LAYOUT_GENERAL, .loadOp=depth_load ? VK_ATTACHMENT_LOAD_OP_LOAD : VK_ATTACHMENT_LOAD_OP_CLEAR,
        .storeOp=VK_ATTACHMENT_STORE_OP_STORE, .clearValue={.depthStencil={clear,0}}};
    VkRect2D area = {.extent={s->width,s->height}};
    VkRenderingInfo render = {.sType=VK_STRUCTURE_TYPE_RENDERING_INFO, .renderArea=area, .layerCount=1,
        .colorAttachmentCount=1, .pColorAttachments=&color, .pDepthAttachment=&depth};
    n->vkCmdBeginRendering(b->command, &render);
    n->vkCmdBindPipeline(b->command, VK_PIPELINE_BIND_POINT_GRAPHICS, p->pipeline);
    VkViewport viewport = {.width=(float)s->width, .height=(float)s->height, .maxDepth=1};
    n->vkCmdSetViewport(b->command, 0, 1, &viewport); n->vkCmdSetScissor(b->command, 0, 1, &area);
    uint64_t address=s->vertices.address+GUARD; NEED(native_push(n,b,p,&address,sizeof(address)));
    VkBindIndexBuffer3InfoKHR indices = {.sType=VK_STRUCTURE_TYPE_BIND_INDEX_BUFFER_3_INFO_KHR,
        .addressRange={s->indices.address+GUARD,8*s->index_bytes}, .addressFlags=VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR,
        .indexType=s->index_bytes==2 ? VK_INDEX_TYPE_UINT16 : VK_INDEX_TYPE_UINT32};
    s->bind_index(b->command, &indices);
    for (unsigned i=0; i<count; ++i) {
        unsigned which=(first+i)%2;
        VkDrawIndirect2InfoKHR draw = {.sType=VK_STRUCTURE_TYPE_DRAW_INDIRECT_2_INFO_KHR,
            .addressRange={.address=s->draws.address+GUARD+20*which, .size=20, .stride=20},
            .addressFlags=VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR, .drawCount=1};
        s->draw_indexed(b->command, &draw);
    }
    n->vkCmdEndRendering(b->command); return 1;
}

static int scene_frame(Scene *s, unsigned mode, unsigned phase, unsigned frame, const char *directory) {
    Native *n=&s->n; NativeBatch *b=&s->batch;
    NEED(native_begin(n,b));
    native_barrier(n,b->command,VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,VK_ACCESS_2_MEMORY_READ_BIT | VK_ACCESS_2_MEMORY_WRITE_BIT,
        VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT,VK_ACCESS_2_SHADER_WRITE_BIT);
    SceneRoot root={s->vertices.address+GUARD,s->indices.address+GUARD,s->draws.address+GUARD,phase,mode==9};
    NEED(native_dispatch(n,b,&s->prepare,(Launch){.x=1,.y=1},&root,sizeof(root)));
    native_barrier(n,b->command,VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT,VK_ACCESS_2_SHADER_WRITE_BIT,
        VK_PIPELINE_STAGE_2_INDEX_INPUT_BIT | VK_PIPELINE_STAGE_2_VERTEX_SHADER_BIT | VK_PIPELINE_STAGE_2_DRAW_INDIRECT_BIT,
        VK_ACCESS_2_INDEX_READ_BIT | VK_ACCESS_2_SHADER_READ_BIT | VK_ACCESS_2_INDIRECT_COMMAND_READ_BIT);
    scene_transition(s);
    if (mode>=6 && mode<=8) {
        NEED(scene_pass(s,0,0,0,1,1,1));
        native_barrier(n,b->command,VK_PIPELINE_STAGE_2_COLOR_ATTACHMENT_OUTPUT_BIT | VK_PIPELINE_STAGE_2_EARLY_FRAGMENT_TESTS_BIT | VK_PIPELINE_STAGE_2_LATE_FRAGMENT_TESTS_BIT,
            VK_ACCESS_2_COLOR_ATTACHMENT_WRITE_BIT | VK_ACCESS_2_DEPTH_STENCIL_ATTACHMENT_WRITE_BIT,
            VK_PIPELINE_STAGE_2_COLOR_ATTACHMENT_OUTPUT_BIT | VK_PIPELINE_STAGE_2_EARLY_FRAGMENT_TESTS_BIT | VK_PIPELINE_STAGE_2_LATE_FRAGMENT_TESTS_BIT,
            VK_ACCESS_2_COLOR_ATTACHMENT_READ_BIT | VK_ACCESS_2_COLOR_ATTACHMENT_WRITE_BIT | VK_ACCESS_2_DEPTH_STENCIL_ATTACHMENT_READ_BIT | VK_ACCESS_2_DEPTH_STENCIL_ATTACHMENT_WRITE_BIT);
        NEED(scene_pass(s,0,mode!=7,mode!=8,1,0,1));
    } else NEED(scene_pass(s,mode>=2 && mode<=4 ? mode-1 : 0,0,0,mode==5 ? 0 : 1,mode>=1 && mode<=4,2));
    NEED(native_image_readback(n,b,&s->color,&s->readback,GUARD));
    VkDeviceMemoryImageCopyKHR region = {.sType=VK_STRUCTURE_TYPE_DEVICE_MEMORY_IMAGE_COPY_KHR,
        .addressRange={s->readback.address+s->pixels+3*GUARD,s->pixels}, .addressFlags=VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR,
        .imageLayout=VK_IMAGE_LAYOUT_GENERAL, .imageSubresource={.aspectMask=VK_IMAGE_ASPECT_DEPTH_BIT,.layerCount=1},
        .imageExtent={s->width,s->height,1}};
    VkCopyDeviceMemoryImageInfoKHR copy = {.sType=VK_STRUCTURE_TYPE_COPY_DEVICE_MEMORY_IMAGE_INFO_KHR,
        .image=s->depth.image, .regionCount=1, .pRegions=&region};
    n->vkCmdCopyImageToMemoryKHR(b->command,&copy);
    NEED(native_copy(n,b,&s->vertices,0,&s->geometry,0,256)
        && native_copy(n,b,&s->indices,0,&s->geometry,256,160)
        && native_copy(n,b,&s->draws,0,&s->geometry,416,168));
    NEED(native_submit(n,b) && native_wait(n,b)); native_batch_destroy(n,b);
    size_t total=2*s->pixels+4*GUARD;
    NEED(native_read(n,&s->readback,0,s->cpu,total));
    char name[128],path[4096]; snprintf(name,sizeof(name),"mode-%u-frame-%u.images",mode,frame);
    NEED(native_path(path,directory,name) && native_file(path,s->cpu,total,1));
    NEED(native_read(n,&s->geometry,0,s->cpu,584));
    snprintf(name,sizeof(name),"mode-%u-frame-%u.geometry",mode,frame);
    NEED(native_path(path,directory,name) && native_file(path,s->cpu,584,1));
    printf("SCENE_FRAME {\"mode\":%u,\"phase\":%u,\"frame\":%u}\n",mode,phase,frame); return 1;
}

static int scene_create(Scene *s, const char *directory) {
    Native *n=&s->n; NEED(native_create(n));
    PFN_vkGetInstanceProcAddr get=(PFN_vkGetInstanceProcAddr)dlsym(n->library,"vkGetInstanceProcAddr"); NEED(get);
    s->bind_index=(PFN_vkCmdBindIndexBuffer3KHR)get(n->instance,"vkCmdBindIndexBuffer3KHR");
    s->draw_indexed=(PFN_vkCmdDrawIndexedIndirect2KHR)get(n->instance,"vkCmdDrawIndexedIndirect2KHR");
    NEED(s->bind_index && s->draw_indexed);
    NEED(scene_shader(n,&s->prepare,2,directory,"compute.spv"));
    for(unsigned i=0;i<4;++i) NEED(scene_raster(s,i,directory));
    NEED(native_image_create(n,&s->color,s->width,s->height) && scene_depth(s));
    NEED(native_buffer_create(n,&s->vertices,256,0)
        && native_buffer_create_usage(n,&s->indices,160,0,VK_BUFFER_USAGE_INDEX_BUFFER_BIT)
        && native_buffer_create(n,&s->draws,168,0) && native_buffer_create(n,&s->upload,256,1)
        && native_buffer_create(n,&s->readback,2*s->pixels+4*GUARD,1) && native_buffer_create(n,&s->geometry,584,1));
    s->cpu=malloc(2*s->pixels+4*GUARD); NEED(s->cpu);
    memset(s->cpu,0xa5,2*s->pixels+4*GUARD);
    NEED(native_write(n,&s->upload,0,s->cpu,256) && native_write(n,&s->readback,0,s->cpu,2*s->pixels+4*GUARD));
    NEED(native_begin(n,&s->batch) && native_copy(n,&s->batch,&s->upload,0,&s->vertices,0,256)
        && native_copy(n,&s->batch,&s->upload,0,&s->indices,0,160) && native_copy(n,&s->batch,&s->upload,0,&s->draws,0,168)
        && native_submit(n,&s->batch) && native_wait(n,&s->batch));
    native_batch_destroy(n,&s->batch); return 1;
}
static void scene_destroy(Scene *s) {
    Native *n=&s->n; native_batch_destroy(n,&s->batch); /* Drain before destroying any backing. */
    native_buffer_destroy(n,&s->geometry); native_buffer_destroy(n,&s->readback); native_buffer_destroy(n,&s->upload);
    native_buffer_destroy(n,&s->draws); native_buffer_destroy(n,&s->indices); native_buffer_destroy(n,&s->vertices);
    native_image_destroy(n,&s->depth); native_image_destroy(n,&s->color);
    for(unsigned i=0;i<4;++i) native_program_destroy(n,&s->raster[i]);
    native_program_destroy(n,&s->prepare); native_destroy(n); free(s->cpu);
}
int main(int argc, char **argv) {
    if(argc!=5 && argc!=6) { fprintf(stderr,"Usage: native width height shaders output-directory [16|32]\n"); return 1; }
    Scene s={.index_bytes=4};
    if(argc==6) {
        if(!strcmp(argv[5],"16")) s.index_bytes=2;
        else if(strcmp(argv[5],"32")) return 1;
    }
    /* Deliberately bounded CLI; large/arbitrary extents are not this control. */
    if(!strcmp(argv[1],"257") && !strcmp(argv[2],"193")) { s.width=257;s.height=193; }
    else if(!strcmp(argv[1],"640") && !strcmp(argv[2],"360")) { s.width=640;s.height=360; }
    else if(!strcmp(argv[1],"1280") && !strcmp(argv[2],"720")) { s.width=1280;s.height=720; }
    else return 1;
    s.pixels=(size_t)s.width*s.height*4; int okay=0;
    if(!scene_create(&s,argv[3])) goto done;
    for(unsigned mode=0;mode<MODES;++mode) for(unsigned frame=0;frame<3;++frame)
        if(!scene_frame(&s,mode,frame==1,frame,argv[4])) goto done;
    okay=1;
done:
    scene_destroy(&s);
    if(okay) puts("Native indexed/depth frames drained; CPU oracle must independently accept outputs");
    return !okay;
}
