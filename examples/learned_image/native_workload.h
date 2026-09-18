/* Included only by the benchmark control. Independent native recording; shared
 * generated shader/root data and extent helpers, no OGPU execution calls. */
#include "extent.h"
#include <application.generated.h>
#include <time.h>

typedef struct { VkShaderModule modules[2]; VkPipeline pipeline; uint32_t root_size; } NativeProgram;
typedef struct {
    VkImage image;
    VkImageView view;
    VkDeviceMemory memory;
    uint32_t width, height, type;
    VkDeviceSize allocated;
} NativeImage;

static int native_image_type(const VkPhysicalDeviceMemoryProperties *memory, uint32_t mask, uint32_t *out) {
    if (memory->memoryTypeCount > VK_MAX_MEMORY_TYPES) return 0;
    int best = -1;
    for (uint32_t i = 0; i < memory->memoryTypeCount; ++i) {
        VkMemoryPropertyFlags f = memory->memoryTypes[i].propertyFlags;
        if (!(mask & (1u << i)) || (f & (VK_MEMORY_PROPERTY_PROTECTED_BIT
            | VK_MEMORY_PROPERTY_DEVICE_COHERENT_BIT_AMD | VK_MEMORY_PROPERTY_LAZILY_ALLOCATED_BIT))) continue;
        int score = 2 * !!(f & VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT) + !(f & VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT);
        if (score >= best) { best = score; *out = i; }
    }
    return best >= 0;
}

static void native_image_destroy(Native *n, NativeImage *image) {
    if (image->view) n->vkDestroyImageView(n->device, image->view, NULL);
    if (image->image) n->vkDestroyImage(n->device, image->image, NULL);
    if (image->memory) n->vkFreeMemory(n->device, image->memory, NULL);
    memset(image, 0, sizeof(*image));
}

static int native_image_create(Native *n, NativeImage *image, uint32_t width, uint32_t height) {
    NEED(!image->image && width && height && width <= n->properties.limits.maxImageDimension2D
         && height <= n->properties.limits.maxImageDimension2D);
    VkImageUsageFlags usage = VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT | VK_IMAGE_USAGE_TRANSFER_SRC_BIT;
    VkImageFormatProperties support;
    VK_TRY(n->vkGetPhysicalDeviceImageFormatProperties(n->physical, VK_FORMAT_R8G8B8A8_UNORM,
        VK_IMAGE_TYPE_2D, VK_IMAGE_TILING_OPTIMAL, usage, 0, &support));
    NEED(width <= support.maxExtent.width && height <= support.maxExtent.height && support.maxExtent.depth >= 1
         && support.maxMipLevels && support.maxArrayLayers && (support.sampleCounts & VK_SAMPLE_COUNT_1_BIT)
         && (uint64_t)width * height * 4 <= support.maxResourceSize);
    image->width = width; image->height = height;
    VkImageCreateInfo create = {.sType=VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO, .imageType=VK_IMAGE_TYPE_2D,
        .format=VK_FORMAT_R8G8B8A8_UNORM, .extent={width, height, 1}, .mipLevels=1, .arrayLayers=1,
        .samples=VK_SAMPLE_COUNT_1_BIT, .tiling=VK_IMAGE_TILING_OPTIMAL, .usage=usage,
        .sharingMode=VK_SHARING_MODE_EXCLUSIVE, .initialLayout=VK_IMAGE_LAYOUT_UNDEFINED};
    VK_TRY(n->vkCreateImage(n->device, &create, NULL, &image->image));
    VkMemoryRequirements requirements;
    n->vkGetImageMemoryRequirements(n->device, image->image, &requirements);
    NEED(native_image_type(&n->memory, requirements.memoryTypeBits, &image->type));
    image->allocated = requirements.size;
    VkMemoryDedicatedAllocateInfo dedicated = {.sType=VK_STRUCTURE_TYPE_MEMORY_DEDICATED_ALLOCATE_INFO, .image=image->image};
    VkMemoryAllocateInfo allocate = {.sType=VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO, .pNext=&dedicated,
        .allocationSize=requirements.size, .memoryTypeIndex=image->type};
    VK_TRY(n->vkAllocateMemory(n->device, &allocate, NULL, &image->memory));
    VK_TRY(n->vkBindImageMemory(n->device, image->image, image->memory, 0));
    VkImageViewCreateInfo view = {.sType=VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO,
        .image=image->image, .viewType=VK_IMAGE_VIEW_TYPE_2D, .format=VK_FORMAT_R8G8B8A8_UNORM,
        .subresourceRange={.aspectMask=VK_IMAGE_ASPECT_COLOR_BIT, .levelCount=1, .layerCount=1}};
    VK_TRY(n->vkCreateImageView(n->device, &view, NULL, &image->view));
    printf("NATIVE_IMAGE {\"logical\":%" PRIu64 ",\"allocated\":%" PRIu64 ",\"type\":%u,\"flags\":%u}\n",
        (uint64_t)width * height * 4, image->allocated, image->type, n->memory.memoryTypes[image->type].propertyFlags);
    return 1;
}

static int native_root_size(Native *n, uint32_t size) { return !(size % 4) && size <= n->max_push_data; }

static int native_local(Native *n, const uint32_t local[3]) {
    uint64_t product = 1;
    for (unsigned i = 0; i < 3; ++i) {
        if (!local[i] || local[i] > n->properties.limits.maxComputeWorkGroupSize[i]
            || product > UINT32_MAX / local[i]) return 0;
        product *= local[i];
    }
    return product <= n->properties.limits.maxComputeWorkGroupInvocations;
}

static void native_program_destroy(Native *n, NativeProgram *p) {
    if (p->pipeline) n->vkDestroyPipeline(n->device, p->pipeline, NULL);
    for (unsigned i = 0; i < 2; ++i) if (p->modules[i]) n->vkDestroyShaderModule(n->device, p->modules[i], NULL);
    memset(p, 0, sizeof(*p));
}

static int native_module(Native *n, const uint32_t *code, size_t size, VkShaderModule *module) {
    NEED(code && size >= 20 && !(size % 4) && code[0] == 0x07230203u);
    VkShaderModuleCreateInfo create = {.sType=VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO, .codeSize=size, .pCode=code};
    VK_TRY(n->vkCreateShaderModule(n->device, &create, NULL, module));
    return 1;
}

static int native_compute(Native *n, NativeProgram *p, const uint32_t *code, size_t size,
                          uint32_t root_size, const uint32_t local[3]) {
    NEED(!p->pipeline && !p->modules[0] && native_root_size(n, root_size) && native_local(n, local));
    p->root_size = root_size;
    if (!native_module(n, code, size, &p->modules[0])) return 0;
    VkSpecializationInfo specialization = {0};
    VkPipelineCreateFlags2CreateInfo flags = {.sType=VK_STRUCTURE_TYPE_PIPELINE_CREATE_FLAGS_2_CREATE_INFO,
        .flags=VK_PIPELINE_CREATE_2_DESCRIPTOR_HEAP_BIT_EXT};
    VkComputePipelineCreateInfo create = {.sType=VK_STRUCTURE_TYPE_COMPUTE_PIPELINE_CREATE_INFO,
        .pNext=&flags, .basePipelineIndex=-1,
        .stage={.sType=VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
            .stage=VK_SHADER_STAGE_COMPUTE_BIT, .module=p->modules[0], .pName="main", .pSpecializationInfo=&specialization}};
    VK_TRY(n->vkCreateComputePipelines(n->device, VK_NULL_HANDLE, 1, &create, NULL, &p->pipeline));
    return 1;
}

static int native_raster(Native *n, NativeProgram *p) {
    NEED(!p->pipeline && !p->modules[0] && !p->modules[1] && fullscreen_push_size == 0
         && native_root_size(n, display_push_size));
    p->root_size = display_push_size;
    if (!native_module(n, fullscreen_code, sizeof(fullscreen_code), &p->modules[0])
        || !native_module(n, display_code, sizeof(display_code), &p->modules[1])) return 0;
    VkSpecializationInfo specialization = {0};
    VkPipelineShaderStageCreateInfo stages[2] = {
        {.sType=VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO, .stage=VK_SHADER_STAGE_VERTEX_BIT,
         .module=p->modules[0], .pName="main", .pSpecializationInfo=&specialization},
        {.sType=VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO, .stage=VK_SHADER_STAGE_FRAGMENT_BIT,
         .module=p->modules[1], .pName="main", .pSpecializationInfo=&specialization}};
    VkPipelineVertexInputStateCreateInfo vertex = {.sType=VK_STRUCTURE_TYPE_PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO};
    VkPipelineInputAssemblyStateCreateInfo assembly = {.sType=VK_STRUCTURE_TYPE_PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO,
        .topology=VK_PRIMITIVE_TOPOLOGY_TRIANGLE_LIST};
    VkPipelineViewportStateCreateInfo viewport = {.sType=VK_STRUCTURE_TYPE_PIPELINE_VIEWPORT_STATE_CREATE_INFO,
        .viewportCount=1, .scissorCount=1};
    VkPipelineRasterizationStateCreateInfo raster = {.sType=VK_STRUCTURE_TYPE_PIPELINE_RASTERIZATION_STATE_CREATE_INFO,
        .polygonMode=VK_POLYGON_MODE_FILL, .frontFace=VK_FRONT_FACE_COUNTER_CLOCKWISE, .lineWidth=1};
    VkPipelineMultisampleStateCreateInfo multisample = {.sType=VK_STRUCTURE_TYPE_PIPELINE_MULTISAMPLE_STATE_CREATE_INFO,
        .rasterizationSamples=VK_SAMPLE_COUNT_1_BIT};
    VkPipelineColorBlendAttachmentState attachment = {.colorWriteMask=VK_COLOR_COMPONENT_R_BIT | VK_COLOR_COMPONENT_G_BIT
        | VK_COLOR_COMPONENT_B_BIT | VK_COLOR_COMPONENT_A_BIT};
    VkPipelineColorBlendStateCreateInfo blend = {.sType=VK_STRUCTURE_TYPE_PIPELINE_COLOR_BLEND_STATE_CREATE_INFO,
        .attachmentCount=1, .pAttachments=&attachment};
    VkDynamicState states[] = {VK_DYNAMIC_STATE_VIEWPORT, VK_DYNAMIC_STATE_SCISSOR};
    VkPipelineDynamicStateCreateInfo dynamic = {.sType=VK_STRUCTURE_TYPE_PIPELINE_DYNAMIC_STATE_CREATE_INFO,
        .dynamicStateCount=2, .pDynamicStates=states};
    VkPipelineCreateFlags2CreateInfo flags = {.sType=VK_STRUCTURE_TYPE_PIPELINE_CREATE_FLAGS_2_CREATE_INFO,
        .flags=VK_PIPELINE_CREATE_2_DESCRIPTOR_HEAP_BIT_EXT};
    VkFormat format = VK_FORMAT_R8G8B8A8_UNORM;
    VkPipelineRenderingCreateInfo rendering = {.sType=VK_STRUCTURE_TYPE_PIPELINE_RENDERING_CREATE_INFO,
        .pNext=&flags, .colorAttachmentCount=1, .pColorAttachmentFormats=&format};
    VkGraphicsPipelineCreateInfo create = {.sType=VK_STRUCTURE_TYPE_GRAPHICS_PIPELINE_CREATE_INFO,
        .pNext=&rendering, .stageCount=2, .pStages=stages, .pVertexInputState=&vertex, .pInputAssemblyState=&assembly,
        .pViewportState=&viewport, .pRasterizationState=&raster, .pMultisampleState=&multisample,
        .pColorBlendState=&blend, .pDynamicState=&dynamic, .basePipelineIndex=-1};
    VK_TRY(n->vkCreateGraphicsPipelines(n->device, VK_NULL_HANDLE, 1, &create, NULL, &p->pipeline));
    return 1;
}

static int native_push(Native *n, NativeBatch *b, NativeProgram *p, const void *root, size_t size) {
    NEED(b->command && !b->value && !b->pending && size == p->root_size && (!size || root));
    if (size) {
        VkPushDataInfoEXT push = {.sType=VK_STRUCTURE_TYPE_PUSH_DATA_INFO_EXT, .data={.address=root, .size=size}};
        n->vkCmdPushDataEXT(b->command, &push);
    }
    return 1;
}

static int native_dispatch(Native *n, NativeBatch *b, NativeProgram *p, Launch grid, const void *root, size_t size) {
    NEED(p->pipeline && grid.x && grid.y && grid.x <= n->properties.limits.maxComputeWorkGroupCount[0]
         && grid.y <= n->properties.limits.maxComputeWorkGroupCount[1] && n->properties.limits.maxComputeWorkGroupCount[2]
         && b->command && !b->value && !b->pending && size == p->root_size && (!size || root));
    n->vkCmdBindPipeline(b->command, VK_PIPELINE_BIND_POINT_COMPUTE, p->pipeline);
    if (!native_push(n, b, p, root, size)) return 0;
    n->vkCmdDispatch(b->command, grid.x, grid.y, 1);
    return 1;
}

static int native_draw(Native *n, NativeBatch *b, NativeProgram *p, NativeImage *image,
                       NativeBuffer *indirect, const void *root, size_t size) {
    NEED(p->pipeline && image->view && indirect->size >= 16 && b->command && !b->value && !b->pending
         && size == p->root_size && root);
    VkImageMemoryBarrier2 discard = {.sType=VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER_2,
        .srcStageMask=VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT, .dstStageMask=VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT,
        .srcAccessMask=VK_ACCESS_2_MEMORY_READ_BIT | VK_ACCESS_2_MEMORY_WRITE_BIT,
        .dstAccessMask=VK_ACCESS_2_MEMORY_READ_BIT | VK_ACCESS_2_MEMORY_WRITE_BIT,
        .oldLayout=VK_IMAGE_LAYOUT_UNDEFINED, .newLayout=VK_IMAGE_LAYOUT_GENERAL,
        .srcQueueFamilyIndex=VK_QUEUE_FAMILY_IGNORED, .dstQueueFamilyIndex=VK_QUEUE_FAMILY_IGNORED,
        .image=image->image, .subresourceRange={.aspectMask=VK_IMAGE_ASPECT_COLOR_BIT, .levelCount=1, .layerCount=1}};
    VkDependencyInfo dependency = {.sType=VK_STRUCTURE_TYPE_DEPENDENCY_INFO, .imageMemoryBarrierCount=1, .pImageMemoryBarriers=&discard};
    n->vkCmdPipelineBarrier2(b->command, &dependency);
    VkRect2D area = {.extent={image->width, image->height}};
    VkRenderingAttachmentInfo attachment = {.sType=VK_STRUCTURE_TYPE_RENDERING_ATTACHMENT_INFO,
        .imageView=image->view, .imageLayout=VK_IMAGE_LAYOUT_GENERAL, .loadOp=VK_ATTACHMENT_LOAD_OP_CLEAR,
        .storeOp=VK_ATTACHMENT_STORE_OP_STORE, .clearValue={.color={.float32={0,0,0,1}}}};
    VkRenderingInfo rendering = {.sType=VK_STRUCTURE_TYPE_RENDERING_INFO, .renderArea=area,
        .layerCount=1, .colorAttachmentCount=1, .pColorAttachments=&attachment};
    n->vkCmdBeginRendering(b->command, &rendering);
    n->vkCmdBindPipeline(b->command, VK_PIPELINE_BIND_POINT_GRAPHICS, p->pipeline);
    VkViewport viewport = {.width=(float)image->width, .height=(float)image->height, .maxDepth=1};
    n->vkCmdSetViewport(b->command, 0, 1, &viewport);
    n->vkCmdSetScissor(b->command, 0, 1, &area);
    if (!native_push(n, b, p, root, size)) return 0;
    VkDrawIndirect2InfoKHR draw = {.sType=VK_STRUCTURE_TYPE_DRAW_INDIRECT_2_INFO_KHR,
        .addressRange={.address=indirect->address, .size=16, .stride=16},
        .addressFlags=VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR, .drawCount=1};
    n->vkCmdDrawIndirect2KHR(b->command, &draw);
    n->vkCmdEndRendering(b->command);
    return 1;
}

static int native_image_readback(Native *n, NativeBatch *b, NativeImage *image, NativeBuffer *dst, VkDeviceSize offset) {
    VkDeviceSize bytes = (VkDeviceSize)image->width * image->height * 4;
    NEED(b->command && !b->value && !b->pending && native_range(dst->size, offset, bytes));
    native_barrier(n, b->command, VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT, VK_ACCESS_2_MEMORY_WRITE_BIT,
        VK_PIPELINE_STAGE_2_TRANSFER_BIT, VK_ACCESS_2_TRANSFER_READ_BIT | VK_ACCESS_2_TRANSFER_WRITE_BIT);
    VkDeviceMemoryImageCopyKHR region = {.sType=VK_STRUCTURE_TYPE_DEVICE_MEMORY_IMAGE_COPY_KHR,
        .addressRange={dst->address + offset, bytes}, .addressFlags=VK_ADDRESS_COMMAND_FULLY_BOUND_BIT_KHR,
        .imageLayout=VK_IMAGE_LAYOUT_GENERAL,
        .imageSubresource={.aspectMask=VK_IMAGE_ASPECT_COLOR_BIT, .layerCount=1},
        .imageExtent={image->width, image->height, 1}};
    VkCopyDeviceMemoryImageInfoKHR copy = {.sType=VK_STRUCTURE_TYPE_COPY_DEVICE_MEMORY_IMAGE_INFO_KHR,
        .image=image->image, .regionCount=1, .pRegions=&region};
    n->vkCmdCopyImageToMemoryKHR(b->command, &copy);
    return 1;
}

/* File/guard code is intentionally small and independent of GPU recording. */
static int native_path(char path[4096], const char *directory, const char *name) {
    int length = snprintf(path, 4096, "%s/%s", directory, name);
    return length >= 0 && length < 4096;
}
static int native_file(const char *path, void *data, size_t size, int write) {
    FILE *file = fopen(path, write ? "wb" : "rb");
    if (!file) { perror(path); return 0; }
    int okay = write ? fwrite(data, 1, size, file) == size
        : fread(data, 1, size, file) == size && fgetc(file) == EOF && !ferror(file);
    if (fclose(file)) okay = 0;
    return okay;
}
static void native_poison(void *data, size_t size) {
    uint32_t word = 0x7fc000a5;
    for (size_t i = 0; i < size; i += 4) memcpy((char *)data + i, &word, 4);
}
static int native_guards(const unsigned char *data, size_t size) {
    if (size < 128 || size % 4) return 0;
    for (unsigned i = 0; i < 64; i += 4) {
        uint32_t a, b; memcpy(&a, data+i, 4); memcpy(&b, data+size-64+i, 4);
        if (a != 0x7fc000a5 || b != 0x7fc000a5) return 0;
    }
    return 1;
}
static double native_now(void) {
    struct timespec t; if (clock_gettime(CLOCK_MONOTONIC, &t)) abort();
    return (double)t.tv_sec * 1000 + (double)t.tv_nsec / 1e6;
}

static int native_workload(int argc, char **argv) {
    enum { INPUT, WEIGHTS, HIDDEN, DENOISED, COLOR, COUNT, GUARD=64 };
    Native n = {0}; NativeBatch batch = {0}; NativeImage image = {0};
    NativeProgram programs[4] = {0}, raster = {0};
    NativeBuffer buffers[COUNT] = {0}, upload = {0}, readback = {0}, draw = {0}, input_b = {0};
    unsigned char *host = NULL, *weights = NULL, *pixels = NULL;
    char path[4096], name[96];
    int result = EXIT_FAILURE;
    double started = native_now();
#define WORK(condition) do { if (!(condition)) { fprintf(stderr, "Native workload check line %d: %s\n", __LINE__, #condition); goto cleanup; } } while (0)
    WORK(argc == 11 && !strcmp(argv[1], "--validate"));
    int resident = !strcmp(argv[2], "resident");
    WORK(resident || !strcmp(argv[2], "end-to-end"));
    argv += 2;
    uint16_t endian = 1; WORK(*(unsigned char *)&endian == 1);
    uint32_t w = extent(argv[2]), h = extent(argv[3]), ow = extent(argv[4]), oh = extent(argv[5]);
    uint32_t count, output_count;
    WORK(image_count(w, h, 8, &count) && image_count(ow, oh, 4, &output_count)
         && resize_axis(w, ow) && resize_axis(h, oh));
    size_t payloads[COUNT] = {(size_t)count*4, 89*4, (size_t)count*32, (size_t)count*4, (size_t)output_count*16};
    size_t sizes[COUNT], offsets[COUNT], final_bytes = (size_t)output_count*4, total = final_bytes+128;
    for (unsigned i = 0; i < COUNT; ++i) { sizes[i] = payloads[i]+128; offsets[i] = total; WORK(add_size(total, sizes[i], &total)); }
    size_t upload_size = sizes[INPUT] > sizes[WEIGHTS] ? sizes[INPUT] : sizes[WEIGHTS], host_size;
    WORK(add_size(upload_size, upload_size, &host_size));
    host = malloc(host_size); weights = malloc(sizes[WEIGHTS]); pixels = malloc(total);
    WORK(host && weights && pixels);
    native_poison(weights, sizes[WEIGHTS]); WORK(native_file(argv[1], weights+GUARD, payloads[WEIGHTS], 0));
    for (unsigned slot = 0; slot < 2; ++slot) {
        native_poison(host+slot*upload_size, sizes[INPUT]);
        WORK(native_path(path, argv[7+slot], "input.f32"));
        WORK(native_file(path, host+slot*upload_size+GUARD, payloads[INPUT], 0));
    }
    WORK(native_create(&n));
    Launch hidden_grid, denoise_grid, process_grid, poison_grid[COUNT] = {0};
    WORK(launch(count*8, hidden_local, n.properties.limits.maxComputeWorkGroupCount, &hidden_grid));
    WORK(launch(count, denoise_local, n.properties.limits.maxComputeWorkGroupCount, &denoise_grid));
    WORK(launch(output_count, process_local, n.properties.limits.maxComputeWorkGroupCount, &process_grid));
    for (unsigned i = HIDDEN; i < COUNT; ++i)
        WORK(launch((uint32_t)(sizes[i]/4), poison_local, n.properties.limits.maxComputeWorkGroupCount, &poison_grid[i]));
#define COMPUTE(index, prefix) WORK(native_compute(&n, &programs[index], prefix##_code, sizeof(prefix##_code), prefix##_push_size, prefix##_local))
    COMPUTE(0, hidden); COMPUTE(1, denoise); COMPUTE(2, process); COMPUTE(3, poison);
#undef COMPUTE
    WORK(native_raster(&n, &raster));
    uint64_t addresses[COUNT];
    for (unsigned i = 0; i < COUNT; ++i) {
        WORK(native_buffer_create(&n, &buffers[i], sizes[i], 0)); addresses[i] = buffers[i].address + GUARD;
    }
    WORK(native_buffer_create(&n, &upload, upload_size, 1));
    WORK(native_buffer_create(&n, &readback, total, 1));
    WORK(native_buffer_create(&n, &draw, 16, 1));
    const uint32_t draw_args[] = {3, 1, 0, 0};
    WORK(native_write(&n, &draw, 0, draw_args, sizeof(draw_args)));
    WORK(native_image_create(&n, &image, ow, oh));
    WORK(native_write(&n, &upload, 0, weights, sizes[WEIGHTS]) && native_begin(&n, &batch));
    WORK(native_copy(&n, &batch, &upload, 0, &buffers[WEIGHTS], 0, sizes[WEIGHTS]));
    for (unsigned i = HIDDEN; i < COUNT; ++i) {
        PoisonArguments root = {.arg_output_data=buffers[i].address, .arg_count=(uint32_t)(sizes[i]/4),
            .arg_dispatch_width=poison_grid[i].stride};
        WORK(native_dispatch(&n, &batch, &programs[3], poison_grid[i], &root, sizeof(root)));
    }
    WORK(native_submit(&n, &batch) && native_wait(&n, &batch)); native_batch_destroy(&n, &batch);
    if (resident) {
        WORK(native_buffer_create(&n, &input_b, sizes[INPUT], 0));
        for (unsigned slot = 0; slot < 2; ++slot) {
            WORK(native_write(&n, &upload, 0, host+slot*upload_size, sizes[INPUT]) && native_begin(&n, &batch));
            WORK(native_copy(&n, &batch, &upload, 0, slot ? &input_b : &buffers[INPUT], 0, sizes[INPUT]));
            WORK(native_submit(&n, &batch) && native_wait(&n, &batch)); native_batch_destroy(&n, &batch);
        }
    }
    native_poison(pixels, total); WORK(native_write(&n, &readback, 0, pixels, total));
    size_t device_bytes = resident ? sizes[INPUT] : 0, host_bytes, cpu_bytes;
    for (unsigned i = 0; i < COUNT; ++i) WORK(add_size(device_bytes, sizes[i], &device_bytes));
    WORK(add_size(upload_size, total, &host_bytes) && add_size(host_bytes, 16, &host_bytes));
    WORK(add_size(host_size, sizes[WEIGHTS], &cpu_bytes) && add_size(cpu_bytes, total, &cpu_bytes));
    printf("MEASUREMENT {\"mode\":\"%s\",\"validation\":true,\"warmups\":0,\"frames\":3,"
        "\"setup_ms\":%.6f,\"device_buffers\":%zu,\"host_buffers\":%zu,\"cpu_payload\":%zu,"
        "\"image_logical\":%zu,\"upload_bytes\":%zu,\"readback_bytes\":%zu}\n",
        resident ? "resident" : "end-to-end", native_now()-started, device_bytes, host_bytes, cpu_bytes,
        final_bytes, resident ? 0 : sizes[INPUT], resident ? 0 : final_bytes);
    HiddenArguments hidden = {.arg_input_data=addresses[INPUT], .arg_weights=addresses[WEIGHTS],
        .arg_output_data=addresses[HIDDEN], .arg_width=w, .arg_height=h, .arg_dispatch_width=hidden_grid.stride};
    DenoiseArguments denoise = {.arg_input_data=addresses[INPUT], .arg_weights=addresses[WEIGHTS],
        .arg_hidden=addresses[HIDDEN], .arg_output_data=addresses[DENOISED], .arg_count=count, .arg_dispatch_width=denoise_grid.stride};
    ProcessArguments process = {.arg_input_data=addresses[DENOISED], .arg_output_data=addresses[COLOR],
        .arg_width=w, .arg_height=h, .arg_out_width=ow, .arg_out_height=oh, .arg_dispatch_width=process_grid.stride};
    DisplayArguments display = {.arg_pixels=addresses[COLOR], .arg_width=ow, .arg_height=oh};
    for (unsigned diagnostic = 0; diagnostic < 2; ++diagnostic) for (unsigned frame = 0; frame < 3; ++frame) {
        unsigned slot = frame%2;
        NativeBuffer *input = resident && slot ? &input_b : &buffers[INPUT];
        hidden.arg_input_data = denoise.arg_input_data = input->address + GUARD;
        if (!resident) WORK(native_write(&n, &upload, 0, host+slot*upload_size, sizes[INPUT]));
        WORK(native_begin(&n, &batch));
        native_barrier(&n, batch.command, VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT | VK_PIPELINE_STAGE_2_FRAGMENT_SHADER_BIT
            | VK_PIPELINE_STAGE_2_TRANSFER_BIT, VK_ACCESS_2_SHADER_READ_BIT | VK_ACCESS_2_SHADER_WRITE_BIT
            | VK_ACCESS_2_TRANSFER_READ_BIT | VK_ACCESS_2_TRANSFER_WRITE_BIT,
            VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT | VK_PIPELINE_STAGE_2_TRANSFER_BIT,
            VK_ACCESS_2_SHADER_READ_BIT | VK_ACCESS_2_SHADER_WRITE_BIT | VK_ACCESS_2_TRANSFER_WRITE_BIT);
        if (!resident) WORK(native_copy(&n, &batch, &upload, 0, &buffers[INPUT], 0, sizes[INPUT]));
        native_barrier(&n, batch.command, VK_PIPELINE_STAGE_2_TRANSFER_BIT, VK_ACCESS_2_TRANSFER_WRITE_BIT,
            VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT, VK_ACCESS_2_SHADER_READ_BIT);
        if (diagnostic) {
            for (unsigned i = HIDDEN; i < COUNT; ++i) {
                PoisonArguments root = {.arg_output_data=buffers[i].address, .arg_count=(uint32_t)(sizes[i]/4),
                    .arg_dispatch_width=poison_grid[i].stride};
                WORK(native_dispatch(&n, &batch, &programs[3], poison_grid[i], &root, sizeof(root)));
            }
            native_barrier(&n, batch.command, VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT, VK_ACCESS_2_SHADER_WRITE_BIT,
                VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT, VK_ACCESS_2_SHADER_WRITE_BIT);
        }
        WORK(native_dispatch(&n, &batch, &programs[0], hidden_grid, &hidden, sizeof(hidden)));
        native_barrier(&n, batch.command, VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT, VK_ACCESS_2_SHADER_WRITE_BIT,
            VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT, VK_ACCESS_2_SHADER_READ_BIT);
        WORK(native_dispatch(&n, &batch, &programs[1], denoise_grid, &denoise, sizeof(denoise)));
        native_barrier(&n, batch.command, VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT, VK_ACCESS_2_SHADER_WRITE_BIT,
            VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT, VK_ACCESS_2_SHADER_READ_BIT);
        WORK(native_dispatch(&n, &batch, &programs[2], process_grid, &process, sizeof(process)));
        native_barrier(&n, batch.command, VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT, VK_ACCESS_2_SHADER_WRITE_BIT,
            VK_PIPELINE_STAGE_2_FRAGMENT_SHADER_BIT, VK_ACCESS_2_SHADER_READ_BIT);
        WORK(native_draw(&n, &batch, &raster, &image, &draw, &display, sizeof(display)));
        if (!resident || diagnostic) WORK(native_image_readback(&n, &batch, &image, &readback, GUARD));
        if (diagnostic) {
            native_barrier(&n, batch.command, VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT | VK_PIPELINE_STAGE_2_TRANSFER_BIT,
                VK_ACCESS_2_SHADER_WRITE_BIT | VK_ACCESS_2_TRANSFER_WRITE_BIT,
                VK_PIPELINE_STAGE_2_TRANSFER_BIT, VK_ACCESS_2_TRANSFER_READ_BIT);
            for (unsigned i = 0; i < COUNT; ++i)
                WORK(native_copy(&n, &batch, i == INPUT ? input : &buffers[i], 0, &readback, offsets[i], sizes[i]));
        }
        WORK(native_submit(&n, &batch) && native_wait(&n, &batch)); native_batch_destroy(&n, &batch);
        if (resident && !diagnostic) {
            WORK(native_begin(&n, &batch) && native_image_readback(&n, &batch, &image, &readback, GUARD));
            WORK(native_submit(&n, &batch) && native_wait(&n, &batch)); native_batch_destroy(&n, &batch);
        }
        WORK(native_read(&n, &readback, 0, pixels, diagnostic ? total : final_bytes+128));
        WORK(native_guards(pixels, final_bytes+128));
        snprintf(name, sizeof(name), "%s-%u-final.rgba", diagnostic ? "diagnostic" : "normal", frame);
        WORK(native_path(path, argv[6], name) && native_file(path, pixels+GUARD, final_bytes, 1));
        if (diagnostic) {
            const char *names[] = {"input", "weights", "hidden", "denoised", "processed"};
            for (unsigned i = 0; i < COUNT; ++i) {
                WORK(native_guards(pixels+offsets[i], sizes[i]));
                snprintf(name, sizeof(name), "diagnostic-%u-%s.f32", frame, names[i]);
                WORK(native_path(path, argv[6], name) && native_file(path, pixels+offsets[i]+GUARD, payloads[i], 1));
            }
            WORK(!memcmp(pixels+offsets[INPUT], host+slot*upload_size, sizes[INPUT]));
            WORK(!memcmp(pixels+offsets[WEIGHTS], weights, sizes[WEIGHTS]));
        }
        printf("Native %s frame %u: guards/input/weights PASS\n", diagnostic ? "diagnostic" : "normal", frame);
    }
    result = EXIT_SUCCESS;
cleanup:
    native_batch_destroy(&n, &batch);
    native_image_destroy(&n, &image); native_program_destroy(&n, &raster);
    for (unsigned i = 0; i < 4; ++i) native_program_destroy(&n, &programs[i]);
    for (unsigned i = 0; i < COUNT; ++i) native_buffer_destroy(&n, &buffers[i]);
    native_buffer_destroy(&n, &draw); native_buffer_destroy(&n, &readback); native_buffer_destroy(&n, &upload);
    native_buffer_destroy(&n, &input_b); native_destroy(&n);
    free(host); free(weights); free(pixels);
    return result;
#undef WORK
}
