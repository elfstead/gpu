/* P3 control: identical producer/transform/consumer with copied or mapped HOST data.
 * Reuse the independently encoded native/public submission machinery, not its workload. */
#define FRONTIER_NO_MAIN
#include "small.c"
#include <assert.h>

typedef struct {
    double start, produce, upload, submit, wait, read, consume, latency;
} HostFrame;
typedef struct {
    Context base;
    unsigned elements, seed[2];
    size_t bytes, stride;
    uint32_t *staging[2];
    uint64_t sums[2], checksum;
    int mapped, shared;
#ifndef FRONTIER_NATIVE
    OgpuHostView views[2];
#endif
#ifdef FRONTIER_NATIVE
    VkSemaphore gate;
    PFN_vkSignalSemaphore signal;
    int opened;
#endif
} HostAccess;

/* End-of-allocation is legal even when not atom aligned. Never wrap arithmetic. */
static int atom_range(uint64_t allocation, uint64_t offset, uint64_t size, uint64_t atom,
                      uint64_t *start, uint64_t *length) {
    if (!atom || !size || offset >= allocation || size > allocation - offset) return 0;
    *start = offset - offset % atom;
    uint64_t end = offset + size, remainder = end % atom;
    if (remainder) {
        uint64_t add = atom - remainder;
        end = add > allocation - end ? allocation : end + add;
    }
    *length = end - *start; return 1;
}
static uint32_t input_word(unsigned seed, unsigned i) {
    return i * 17u + (seed ? UINT32_C(0x80001234) : 1u);
}
static void produce_words(uint32_t *p, unsigned elements, unsigned seed) {
    for (unsigned i = 0; i < GUARD_WORDS; ++i) p[i] = p[GUARD_WORDS + elements + i] = sentinel;
    for (unsigned i = 0; i < elements; ++i) p[GUARD_WORDS + i] = input_word(seed, i);
}
static uint64_t consume_words(const uint32_t *p, unsigned elements) {
    uint64_t sum = 0;
    for (unsigned i = 0; i < elements; ++i) sum += p[GUARD_WORDS + i];
    return sum;
}
static int verify_words(const uint32_t *p, unsigned elements, unsigned seed) {
    for (unsigned i = 0; i < GUARD_WORDS; ++i)
        CHECK(p[i] == sentinel && p[GUARD_WORDS + elements + i] == sentinel);
    for (unsigned i = 0; i < elements; ++i)
        CHECK(p[GUARD_WORDS + i] == expected(input_word(seed, i), 1));
    return 1;
}
#ifdef FRONTIER_NATIVE
static NativeBuffer *backing(HostAccess *h, unsigned i) {
    return &h->base.slots[h->shared ? 0 : i].buffer;
}
static uint64_t byte_offset(HostAccess *h, unsigned i) { return h->shared ? i * h->stride : 0; }
static int sync_range(HostAccess *h, unsigned i, int flush) {
    Native *n = &h->base.native; NativeBuffer *b = backing(h, i);
    uint64_t offset, size;
    CHECK(atom_range(b->allocated, byte_offset(h, i), h->bytes,
        n->properties.limits.nonCoherentAtomSize, &offset, &size));
    if (h->shared) CHECK(offset >= i * h->stride && offset + size <= (i + 1) * h->stride);
    if (b->coherent) return 1;
    VkMappedMemoryRange range = {.sType=VK_STRUCTURE_TYPE_MAPPED_MEMORY_RANGE,
        .memory=b->memory, .offset=offset, .size=size};
    VK_TRY(flush ? n->vkFlushMappedMemoryRanges(n->device, 1, &range)
                 : n->vkInvalidateMappedMemoryRanges(n->device, 1, &range));
    return 1;
}
#endif
static uint32_t *host_pointer(HostAccess *h, unsigned i) {
#ifdef FRONTIER_NATIVE
    if (h->mapped) return (uint32_t *)((char *)backing(h, i)->mapped + byte_offset(h, i));
#else
    if (h->mapped) return (uint32_t *)((char *)h->views[h->shared ? 0 : i].data + (h->shared ? i * h->stride : 0));
#endif
    return h->staging[i];
}
static int upload(HostAccess *h, unsigned i) {
    Context *c = &h->base;
    CHECK(!c->slots[i].pending);
#ifdef FRONTIER_NATIVE
    if (h->mapped && backing(h, i)->coherent) return 1;
    if (!h->mapped) memcpy(backing(h, i)->mapped, h->staging[i], h->bytes);
    return sync_range(h, i, 1);
#else
    if (h->mapped && h->views[h->shared ? 0 : i].coherent) return 1;
    if (h->mapped) { API(ogpu_buffer_host_flush(c->slots[h->shared ? 0 : i].buffer,
        h->shared ? i * h->stride : 0, h->bytes, &c->error)); return 1; }
    API(ogpu_buffer_write(c->slots[i].buffer, 0, h->staging[i], h->bytes, &c->error)); return 1;
#endif
}
static int download(HostAccess *h, unsigned i) {
    Context *c = &h->base;
    CHECK(!c->slots[i].pending);
#ifdef FRONTIER_NATIVE
    if (h->mapped && backing(h, i)->coherent) return 1;
    CHECK(sync_range(h, i, 0));
    if (!h->mapped) memcpy(h->staging[i], backing(h, i)->mapped, h->bytes);
#else
    if (h->mapped && h->views[h->shared ? 0 : i].coherent) return 1;
    if (h->mapped) { API(ogpu_buffer_host_invalidate(c->slots[h->shared ? 0 : i].buffer,
        h->shared ? i * h->stride : 0, h->bytes, &c->error)); return 1; }
    API(ogpu_buffer_read(c->slots[i].buffer, 0, h->staging[i], h->bytes, &c->error));
#endif
    return 1;
}
static int encode(HostAccess *h, unsigned i) {
    Context *c = &h->base; Slot *s = &c->slots[i];
    unsigned groups = (h->elements + transform_local[0] - 1) / transform_local[0];
#ifdef FRONTIER_NATIVE
    Native *n = &c->native;
    CHECK(groups <= n->properties.limits.maxComputeWorkGroupCount[0]);
    CHECK(native_begin(n, &s->batch));
    native_barrier(n, s->batch.command, VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT, VK_ACCESS_2_SHADER_WRITE_BIT,
        VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT, VK_ACCESS_2_SHADER_READ_BIT | VK_ACCESS_2_SHADER_WRITE_BIT);
    CHECK(native_dispatch(n, &s->batch, &c->program, (Launch){.x=groups, .y=1}, &s->root, sizeof(s->root)));
    native_barrier(n, s->batch.command, VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT, VK_ACCESS_2_MEMORY_WRITE_BIT,
        VK_PIPELINE_STAGE_2_HOST_BIT, VK_ACCESS_2_HOST_READ_BIT);
    VK_TRY(n->vkEndCommandBuffer(s->batch.command));
#else
    API(ogpu_batch_create(c->device, &s->batch, &c->error));
    API(ogpu_batch_barrier(s->batch, OGPU_ACCESS_COMPUTE_WRITE,
        OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_COMPUTE_WRITE, &c->error));
    API(ogpu_batch_dispatch(s->batch, c->kernel, groups, 1, 1, &s->root, sizeof(s->root), &c->error));
    API(ogpu_batch_retain_buffer(s->batch, c->slots[h->shared ? 0 : i].buffer, &c->error));
    API(ogpu_batch_compile(s->batch, &s->list, &c->error));
    ogpu_batch_destroy(s->batch); s->batch = NULL;
#endif
    return 1;
}
static int host_create(HostAccess *h) {
    Context *c = &h->base;
    h->bytes = (h->elements + 2 * GUARD_WORDS) * sizeof(uint32_t); h->stride = h->bytes;
#ifdef FRONTIER_NATIVE
    Native *n = &c->native;
    CHECK(native_create(n));
    CHECK(native_compute(n, &c->program, transform_code, sizeof(transform_code), sizeof(TransformArguments), transform_local));
    uint64_t atom = n->properties.limits.nonCoherentAtomSize;
    CHECK(atom && atom <= SIZE_MAX - h->bytes);
    if (h->shared && h->stride % atom) h->stride += atom - h->stride % atom;
    CHECK(h->stride <= SIZE_MAX / c->count);
    printf("HOST_MEMORY {\"atom\":%" PRIu64 ",\"types\":[", atom);
    for (unsigned i = 0; i < n->memory.memoryTypeCount; ++i)
        printf("%s%u", i ? "," : "", n->memory.memoryTypes[i].propertyFlags);
    puts("]}");
#else
    API(ogpu_probe_create(OGPU_ABI_VERSION, &c->probe, &c->error));
    uint32_t count = 0; API(ogpu_probe_device_count(c->probe, &count)); CHECK(count == 1);
    API(ogpu_device_create_graphics(c->probe, 0, &c->device, &c->error));
    OgpuDeviceInfo info; API(ogpu_probe_device_info(c->probe, 0, &info));
    printf("DEVICE {\"vendor\":%u,\"device\":%u,\"api\":[%u,%u,%u]}\n", info.vendor_id, info.device_id,
        info.vulkan_api_major, info.vulkan_api_minor, info.vulkan_api_patch);
    OgpuCapabilities capabilities; API(ogpu_device_capabilities(c->device, &capabilities, &c->error));
    OgpuDeviceLimits limits; API(ogpu_device_limits(c->device, &limits, &c->error));
    CHECK(transform_compatible(&capabilities, &limits));
    CHECK((h->elements + transform_local[0] - 1) / transform_local[0] <= limits.max_dispatch[0]);
    OgpuShaderDesc shader = transform_shader();
    API(ogpu_kernel_create(c->device, &shader, sizeof(TransformArguments), &c->kernel, &c->error));
#endif
    for (unsigned seed = 0; seed < 2; ++seed)
        for (unsigned i = 0; i < h->elements; ++i) h->sums[seed] += expected(input_word(seed, i), 1);
    for (unsigned i = 0; i < c->count; ++i) {
        Slot *s = &c->slots[i]; uint64_t address;
#ifdef FRONTIER_NATIVE
        if (!h->shared || !i) CHECK(native_buffer_create(n, &s->buffer, h->shared ? c->count * h->stride : h->bytes, 1));
        address = backing(h, i)->address + byte_offset(h, i);
#else
        if (!h->shared || !i) {
            API(ogpu_buffer_create(c->device, h->shared ? c->count * h->stride : h->bytes, OGPU_MEMORY_HOST, &s->buffer, &c->error));
            if (h->mapped) {
                API(ogpu_buffer_host_view(s->buffer, &h->views[i], &c->error));
                CHECK(h->views[i].data && h->views[i].alignment && h->views[i].access_granularity);
                CHECK(h->views[i].coherent <= 1 && (uintptr_t)h->views[i].data % h->views[i].alignment == 0);
                /* Fixed-budget protocol: reject rather than silently changing allocation count/size. */
                CHECK(!h->shared || h->stride % h->views[i].access_granularity == 0);
                printf("HOST_VIEW {\"size\":%" PRIu64 ",\"alignment\":%" PRIu64 ",\"granularity\":%" PRIu64 ",\"coherent\":%" PRIu64 "}\n",
                    h->views[i].size_bytes, h->views[i].alignment, h->views[i].access_granularity, h->views[i].coherent);
            }
        }
        API(ogpu_buffer_device_address(c->slots[h->shared ? 0 : i].buffer, &address, &c->error));
        if (h->shared) address += i * h->stride;
#endif
        if (!h->mapped) { h->staging[i] = malloc(h->bytes); CHECK(h->staging[i]); }
        s->root = (TransformArguments){.arg_data=address + GUARD_WORDS * 4, .arg_count=h->elements};
        CHECK(encode(h, i));
    }
    return 1;
}
static int host_retire(HostAccess *h, unsigned i, HostFrame *frames, int validate) {
    Context *c = &h->base; Slot *s = &c->slots[i];
    if (!s->pending) return 1;
    HostFrame *f = &frames[s->sample_index]; double t = clock_ms();
    CHECK(wait_slot(c, s)); double waited = clock_ms();
    CHECK(download(h, i)); double read = clock_ms();
    uint64_t sum = consume_words(host_pointer(h, i), h->elements); double consumed = clock_ms();
    h->checksum += sum; CHECK(sum == h->sums[h->seed[i]]);
    f->wait = waited - t; f->read = read - waited; f->consume = consumed - read; f->latency = consumed - f->start;
    if (validate) CHECK(verify_words(host_pointer(h, i), h->elements, h->seed[i]));
    return 1;
}
static int host_window(HostAccess *h, unsigned count, HostFrame *frames, int validate, double *wall) {
    Context *c = &h->base; double start = clock_ms();
    for (unsigned i = 0; i < count; ++i) {
        unsigned index = i % c->count; Slot *s = &c->slots[index]; HostFrame *f = &frames[i];
        CHECK(host_retire(h, index, frames, validate));
        s->sample_index = i; h->seed[index] = s->submitted % 2; f->start = clock_ms();
        produce_words(host_pointer(h, index), h->elements, h->seed[index]); double produced = clock_ms();
        CHECK(upload(h, index)); double uploaded = clock_ms();
        CHECK(submit_slot(c, s)); double submitted = clock_ms();
        f->produce = produced - f->start; f->upload = uploaded - produced; f->submit = submitted - uploaded;
    }
    unsigned first = count > c->count ? count - c->count : 0;
    for (unsigned i = first; i < count; ++i) CHECK(host_retire(h, i % c->count, frames, validate));
    *wall = clock_ms() - start;
    for (unsigned i = 0; i < c->count; ++i) {
        CHECK(download(h, i)); CHECK(verify_words(host_pointer(h, i), h->elements, h->seed[i]));
    }
    return 1;
}

#ifdef FRONTIER_NATIVE
static HostAccess *gated_host;
static PFN_vkQueueSubmit2 ungated_submit;
static int open_gate(HostAccess *h);
static VkResult VKAPI_CALL gated_submit(VkQueue queue, uint32_t count, const VkSubmitInfo2 *info, VkFence fence) {
    assert(count == 1 && info->waitSemaphoreInfoCount == 0);
    VkSemaphoreSubmitInfo gate = {.sType=VK_STRUCTURE_TYPE_SEMAPHORE_SUBMIT_INFO,
        .semaphore=gated_host->gate, .value=1, .stageMask=VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT};
    VkSubmitInfo2 submit = *info; submit.waitSemaphoreInfoCount = 1; submit.pWaitSemaphoreInfos = &gate;
    VkResult result = ungated_submit(queue, count, &submit, fence);
    /* submit_slot drains unexpected failures; unblock before that drain, too. */
    if (result != VK_SUCCESS && !open_gate(gated_host)) abort();
    return result;
}
static int open_gate(HostAccess *h) {
    if (h->gate && !h->opened) {
        VkSemaphoreSignalInfo signal = {.sType=VK_STRUCTURE_TYPE_SEMAPHORE_SIGNAL_INFO, .semaphore=h->gate, .value=1};
        VK_TRY(h->signal(h->base.native.device, &signal)); h->opened = 1;
    }
    return 1;
}
static int gate_check(HostAccess *h) {
    Context *c = &h->base; Native *n = &c->native; HostFrame frames[1] = {{0}};
    PFN_vkGetInstanceProcAddr get = (PFN_vkGetInstanceProcAddr)dlsym(n->library, "vkGetInstanceProcAddr");
    CHECK(get); h->signal = (PFN_vkSignalSemaphore)get(n->instance, "vkSignalSemaphore"); CHECK(h->signal);
    VkSemaphoreTypeCreateInfo type = {.sType=VK_STRUCTURE_TYPE_SEMAPHORE_TYPE_CREATE_INFO, .semaphoreType=VK_SEMAPHORE_TYPE_TIMELINE};
    VkSemaphoreCreateInfo info = {.sType=VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO, .pNext=&type};
    VK_TRY(n->vkCreateSemaphore(n->device, &info, NULL, &h->gate));
    for (unsigned i = 0; i < 2; ++i) {
        produce_words(host_pointer(h, i), h->elements, 0); CHECK(upload(h, i));
        if (i == 1) { gated_host = h; ungated_submit = n->vkQueueSubmit2; n->vkQueueSubmit2 = gated_submit; }
        int accepted = submit_slot(c, &c->slots[i]);
        if (i == 1) n->vkQueueSubmit2 = ungated_submit;
        CHECK(accepted);
        if (!i) CHECK(host_retire(h, 0, frames, 1));
    }
    VkSemaphoreWaitInfo wait = {.sType=VK_STRUCTURE_TYPE_SEMAPHORE_WAIT_INFO,
        .semaphoreCount=1, .pSemaphores=&n->timeline, .pValues=&c->slots[1].batch.value};
    CHECK(n->vkWaitSemaphores(n->device, &wait, 0) == VK_TIMEOUT);
    /* Slot 0 is complete, slot 1 provably pending in the same allocation for shared.
     * Read old output then produce/flush new input WITHOUT opening the gate. */
    CHECK(download(h, 0) && verify_words(host_pointer(h, 0), h->elements, 0));
    h->seed[0] = 1; produce_words(host_pointer(h, 0), h->elements, 1); CHECK(upload(h, 0));
    CHECK(n->vkWaitSemaphores(n->device, &wait, 0) == VK_TIMEOUT);
    CHECK(open_gate(h) && host_retire(h, 1, frames, 1));
    CHECK(submit_slot(c, &c->slots[0]) && host_retire(h, 0, frames, 1));
    puts("HOST_GATE disjoint CPU read/write/flush while other range pending PASS"); return 1;
}
#endif
static void host_destroy(HostAccess *h) {
#ifdef FRONTIER_NATIVE
    /* Failure cleanup opens the gate before any destructor can wait behind it. */
    if (!open_gate(h)) abort();
    for (unsigned i = 0; i < h->base.count; ++i) (void)wait_slot(&h->base, &h->base.slots[i]);
    if (h->gate) h->base.native.vkDestroySemaphore(h->base.native.device, h->gate, NULL);
#endif
    destroy(&h->base);
    for (unsigned i = 0; i < 2; ++i) free(h->staging[i]);
}
static int host_selftest(void) {
    uint64_t offset, size;
    CHECK(atom_range(1000, 260, 1, 256, &offset, &size) && offset == 256 && size == 256);
    CHECK(atom_range(1000, 900, 100, 256, &offset, &size) && offset == 768 && size == 232);
    CHECK(atom_range(UINT64_MAX, UINT64_MAX - 1, 1, 256, &offset, &size) && size == 255);
    CHECK(!atom_range(100, 99, 2, 16, &offset, &size) && !atom_range(100, 0, 0, 16, &offset, &size));
    CHECK(!atom_range(100, 0, 1, 0, &offset, &size));
    for (uint64_t atom = 1; atom <= 512; atom *= 2) {
        uint64_t stride = 388 + (atom - 388 % atom) % atom;
        CHECK(atom_range(2 * stride, 0, 388, atom, &offset, &size) && offset == 0 && size <= stride);
        CHECK(atom_range(2 * stride, stride, 388, atom, &offset, &size) && offset == stride && size <= stride);
    }
    uint32_t words[WORDS];
    for (unsigned seed = 0; seed < 2; ++seed) {
        produce_words(words, ELEMENTS, seed); uint64_t sum = 0;
        for (unsigned i = 0; i < ELEMENTS; ++i) { words[GUARD_WORDS + i] = words[GUARD_WORDS + i] * 3u + 7u; sum += words[GUARD_WORDS + i]; }
        CHECK(verify_words(words, ELEMENTS, seed) && consume_words(words, ELEMENTS) == sum);
        words[0] ^= 1; CHECK(!verify_words(words, ELEMENTS, seed)); words[0] ^= 1;
        words[GUARD_WORDS + ELEMENTS - 1] ^= 1; CHECK(!verify_words(words, ELEMENTS, seed));
    }
    puts("HOST_ACCESS atom boundaries/overflow/producer/oracle/consumer PASS"); return 1;
}
#ifndef HOST_ACCESS_NO_MAIN
int main(int argc, char **argv) {
    (void)create; (void)window; (void)selftest; (void)atom_range;
    if (argc == 2 && !strcmp(argv[1], "--selftest")) return !host_selftest();
    if (argc != 6) { fprintf(stderr, "Usage: host-access native-copy|mapped|shared|ogpu slots kib frames validate|measure|gate\n"); return 1; }
    HostAccess h = {0}; Context *c = &h.base;
    const char *policy = argv[1]; c->count = number(argv[2]); unsigned kib = number(argv[3]), frames = number(argv[4]);
    int validate = !strcmp(argv[5], "validate"), gate = !strcmp(argv[5], "gate");
    if ((c->count != 1 && c->count != 2) || (kib != 64 && kib != 4096) || frames < c->count ||
        (!validate && !gate && strcmp(argv[5], "measure"))) return 1;
    h.elements = kib * 1024 / 4;
#ifdef FRONTIER_NATIVE
    if (strcmp(policy, "native-copy") && strcmp(policy, "mapped") && strcmp(policy, "shared")) return 1;
    h.mapped = strcmp(policy, "native-copy") != 0; h.shared = !strcmp(policy, "shared"); c->policy = "replay";
    if ((h.shared && c->count != 2) || (gate && (!h.mapped || c->count != 2))) return 1;
#else
    if ((strcmp(policy, "ogpu") && strcmp(policy, "ogpu-mapped") && strcmp(policy, "ogpu-shared")) || gate) return 1;
    h.mapped = strcmp(policy, "ogpu") != 0; h.shared = !strcmp(policy, "ogpu-shared");
    if (h.shared && c->count != 2) return 1;
    c->policy = "compiled";
#endif
    HostFrame *samples = calloc(frames > 100 ? frames : 100, sizeof(*samples)); if (!samples) return 1;
    int okay = 0; double setup = clock_ms(), wall = 0;
    if (!host_create(&h)) goto cleanup;
    setup = clock_ms() - setup;
#ifdef FRONTIER_NATIVE
    if (gate) { okay = gate_check(&h); goto cleanup; }
#endif
    if (!validate && !host_window(&h, 100, samples, 0, &wall)) goto cleanup;
    memset(samples, 0, (frames > 100 ? frames : 100) * sizeof(*samples)); h.checksum = 0;
    if (!host_window(&h, frames, samples, validate, &wall)) goto cleanup;
    printf("HOST_RESULT {\"policy\":\"%s\",\"slots\":%u,\"kib\":%u,\"frames\":%u,\"warmups\":%u,"
        "\"validation\":%s,\"requested_bytes\":%zu,\"staging_bytes\":%zu,\"stride\":%zu,\"checksum\":%" PRIu64 ","
        "\"setup_ms\":%.9f,\"wall_ms\":%.9f}\n", policy, c->count, kib, frames, validate ? 0 : 100,
        validate ? "true" : "false", h.stride * c->count, h.mapped ? 0 : h.bytes * c->count, h.stride, h.checksum, setup, wall);
    if (!validate) for (unsigned i = 0; i < frames; ++i) {
        HostFrame *f = &samples[i];
        printf("HOST_FRAME {\"index\":%u,\"produce_ms\":%.9f,\"upload_ms\":%.9f,\"submit_ms\":%.9f,"
            "\"wait_ms\":%.9f,\"read_ms\":%.9f,\"consume_ms\":%.9f,\"latency_ms\":%.9f}\n",
            i, f->produce, f->upload, f->submit, f->wait, f->read, f->consume, f->latency);
    }
    okay = 1;
cleanup:
    host_destroy(&h); free(samples);
    if (okay) puts("HOST_ACCESS full outputs/guards/checksums PASS; all slots drained");
    return !okay;
}
#endif
