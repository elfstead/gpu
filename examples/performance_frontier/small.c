/* Host-sensitive expressibility control. Native controls never link OGPU.
 * Shared scheduling/oracle, independent public/native command encoding. */
#define _POSIX_C_SOURCE 200809L
#ifdef FRONTIER_NATIVE
#define main learned_image_native_main
#include "../learned_image/native.c"
#undef main
#else
#include "ogpu.h"
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#endif
#include "../compiler/transform.generated.h"
#include <errno.h>

enum { ELEMENTS = 65, GUARD_WORDS = 16, WORDS = ELEMENTS + 2 * GUARD_WORDS, MAX_SLOTS = 3 };
static const uint32_t sentinel = UINT32_C(0xcafef00d);
#define CHECK(x) do { if (!(x)) { fprintf(stderr, "Frontier check line %d: %s\n", __LINE__, #x); return 0; } } while (0)
#ifndef FRONTIER_NATIVE
#define API(x) do { OgpuResult r_ = (x); if (r_ != OGPU_SUCCESS) { \
    fprintf(stderr, "%s: %d %s\n", #x, r_, c->error.message); return 0; } } while (0)
#endif

typedef struct {
#ifdef FRONTIER_NATIVE
    NativeBuffer buffer;
    NativeBatch batch;
#else
    OgpuBuffer *buffer;
    OgpuBatch *batch;
    OgpuCompletion *completion;
#endif
    TransformArguments root;
    unsigned submitted, sample_index;
    int pending;
} Slot;
typedef struct {
#ifdef FRONTIER_NATIVE
    Native native;
    NativeProgram program;
    PFN_vkResetCommandPool reset_pool;
#else
    OgpuProbe *probe;
    OgpuDevice *device;
    OgpuKernel *kernel;
    OgpuError error;
#endif
    Slot slots[MAX_SLOTS];
    unsigned count, dispatches;
    const char *policy;
} Context;
typedef struct { double start, record, submit, wait, latency; } Frame;

static double clock_ms(void) {
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t)) abort();
    return (double)t.tv_sec * 1000.0 + (double)t.tv_nsec / 1e6;
}
static unsigned number(const char *text) {
    if (!text || *text < '0' || *text > '9') return 0;
    errno = 0; char *end;
    unsigned long value = strtoul(text, &end, 10);
    return !errno && !*end && value <= 10000 ? (unsigned)value : 0;
}
/* Affine exponentiation modulo 2^32, independent of GPU execution/scheduling. */
static uint32_t expected(uint32_t value, uint64_t times) {
    uint32_t mul = 3, add = 7;
    while (times) {
        if (times & 1) value = value * mul + add;
        add = add * mul + add; mul *= mul; times >>= 1;
    }
    return value;
}
static uint32_t initial(unsigned slot, unsigned element) { return 101u * slot + element * 17u + 1u; }

static int write_slot(Context *c, Slot *s, const uint32_t *data) {
#ifdef FRONTIER_NATIVE
    return native_write(&c->native, &s->buffer, 0, data, WORDS * 4);
#else
    API(ogpu_buffer_write(s->buffer, 0, data, WORDS * 4, &c->error)); return 1;
#endif
}
static int verify(Context *c, unsigned slot) {
    Slot *s = &c->slots[slot]; uint32_t data[WORDS];
    CHECK(!s->pending);
#ifdef FRONTIER_NATIVE
    CHECK(native_read(&c->native, &s->buffer, 0, data, sizeof(data)));
#else
    API(ogpu_buffer_read(s->buffer, 0, data, sizeof(data), &c->error));
#endif
    for (unsigned i = 0; i < WORDS; ++i) {
        uint32_t want = i < GUARD_WORDS || i >= GUARD_WORDS + ELEMENTS ? sentinel
            : expected(initial(slot, i - GUARD_WORDS), (uint64_t)s->submitted * c->dispatches);
        if (data[i] != want) {
            fprintf(stderr, "Mismatch slot=%u submitted=%u word=%u got=%u expected=%u\n",
                    slot, s->submitted, i, data[i], want); return 0;
        }
    }
    return 1;
}

static int record_slot(Context *c, Slot *s) {
    CHECK(!s->pending);
#ifdef FRONTIER_NATIVE
    Native *n = &c->native; NativeBatch *b = &s->batch;
    if (!strcmp(c->policy, "replay") && b->command) return 1;
    if (b->pool) {
        CHECK(!strcmp(c->policy, "reset"));
        VK_TRY(c->reset_pool(n->device, b->pool, 0));
        VkCommandBufferBeginInfo begin = {.sType=VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO};
        VK_TRY(n->vkBeginCommandBuffer(b->command, &begin));
        native_barrier(n, b->command, VK_PIPELINE_STAGE_2_HOST_BIT, VK_ACCESS_2_HOST_WRITE_BIT,
            VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT, VK_ACCESS_2_MEMORY_READ_BIT | VK_ACCESS_2_MEMORY_WRITE_BIT);
    } else CHECK(native_begin(n, b));
    for (unsigned i = 0; i < c->dispatches; ++i) {
        /* Includes the previous submission's writes to this slot. */
        native_barrier(n, b->command, VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT, VK_ACCESS_2_SHADER_WRITE_BIT,
            VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT, VK_ACCESS_2_SHADER_READ_BIT | VK_ACCESS_2_SHADER_WRITE_BIT);
        CHECK(native_dispatch(n, b, &c->program,
            (Launch){.x=(ELEMENTS + transform_local[0] - 1) / transform_local[0], .y=1}, &s->root, sizeof(s->root)));
    }
    native_barrier(n, b->command, VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT, VK_ACCESS_2_MEMORY_WRITE_BIT,
        VK_PIPELINE_STAGE_2_HOST_BIT, VK_ACCESS_2_HOST_READ_BIT);
    VK_TRY(n->vkEndCommandBuffer(b->command));
#else
    API(ogpu_batch_create(c->device, &s->batch, &c->error));
    for (unsigned i = 0; i < c->dispatches; ++i) {
        API(ogpu_batch_barrier(s->batch, OGPU_ACCESS_COMPUTE_WRITE,
            OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_COMPUTE_WRITE, &c->error));
        API(ogpu_batch_dispatch(s->batch, c->kernel, (ELEMENTS + transform_local[0] - 1) / transform_local[0],
            1, 1, &s->root, sizeof(s->root), &c->error));
    }
#endif
    return 1;
}
static int submit_slot(Context *c, Slot *s) {
    CHECK(!s->pending);
#ifdef FRONTIER_NATIVE
    Native *n = &c->native;
    CHECK(n->next_value < UINT64_MAX && n->next_value - n->observed_value < n->max_difference);
    uint64_t value = ++n->next_value;
    VkCommandBufferSubmitInfo command = {.sType=VK_STRUCTURE_TYPE_COMMAND_BUFFER_SUBMIT_INFO, .commandBuffer=s->batch.command};
    VkSemaphoreSubmitInfo signal = {.sType=VK_STRUCTURE_TYPE_SEMAPHORE_SUBMIT_INFO,
        .semaphore=n->timeline, .value=value, .stageMask=VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT};
    VkSubmitInfo2 submit = {.sType=VK_STRUCTURE_TYPE_SUBMIT_INFO_2, .commandBufferInfoCount=1,
        .pCommandBufferInfos=&command, .signalSemaphoreInfoCount=1, .pSignalSemaphoreInfos=&signal};
    VkResult result = n->vkQueueSubmit2(n->queue, 1, &submit, VK_NULL_HANDLE);
    if (result != VK_SUCCESS) {
        /* Never wait on a rejected timeline value. Drain before teardown. */
        native_drain(n); return vk_ok(result, "frontier submit");
    }
    s->batch.value = value;
#else
    API(ogpu_batch_submit(s->batch, &s->completion, &c->error));
    ogpu_batch_destroy(s->batch); s->batch = NULL;
#endif
    s->pending = 1; ++s->submitted; return 1;
}
static int wait_slot(Context *c, Slot *s) {
    if (!s->pending) return 1;
#ifdef FRONTIER_NATIVE
    Native *n = &c->native;
    VkSemaphoreWaitInfo wait = {.sType=VK_STRUCTURE_TYPE_SEMAPHORE_WAIT_INFO,
        .semaphoreCount=1, .pSemaphores=&n->timeline, .pValues=&s->batch.value};
    VkResult result = n->vkWaitSemaphores(n->device, &wait, UINT64_MAX);
    if (result != VK_SUCCESS && result != VK_ERROR_DEVICE_LOST) native_drain(n);
    s->pending = 0;
    if (result == VK_SUCCESS && s->batch.value > n->observed_value) n->observed_value = s->batch.value;
    s->batch.value = 0;
    if (!strcmp(c->policy, "fresh")) native_batch_destroy(n, &s->batch);
    return vk_ok(result, "frontier wait");
#else
    OgpuResult result = ogpu_completion_wait(s->completion, &c->error);
    /* Wait drains even on a non-loss error; destruction does not release pending work. */
    ogpu_completion_destroy(s->completion); s->completion = NULL; s->pending = 0;
    if (result != OGPU_SUCCESS) { fprintf(stderr, "Frontier wait: %d %s\n", result, c->error.message); return 0; }
    return 1;
#endif
}
static void destroy(Context *c) {
    /* Drain ALL slots before destroying any resource. */
    for (unsigned i = 0; i < c->count; ++i) (void)wait_slot(c, &c->slots[i]);
    for (unsigned i = 0; i < c->count; ++i) {
#ifdef FRONTIER_NATIVE
        native_batch_destroy(&c->native, &c->slots[i].batch);
        native_buffer_destroy(&c->native, &c->slots[i].buffer);
#else
        ogpu_batch_destroy(c->slots[i].batch);
        ogpu_buffer_destroy(c->slots[i].buffer);
#endif
    }
#ifdef FRONTIER_NATIVE
    native_program_destroy(&c->native, &c->program); native_destroy(&c->native);
#else
    ogpu_kernel_destroy(c->kernel); ogpu_device_destroy(c->device); ogpu_probe_destroy(c->probe);
#endif
}
static int create(Context *c) {
#ifdef FRONTIER_NATIVE
    Native *n = &c->native;
    CHECK(native_create(n));
    PFN_vkGetInstanceProcAddr instance_get = (PFN_vkGetInstanceProcAddr)dlsym(n->library, "vkGetInstanceProcAddr");
    CHECK(instance_get);
    PFN_vkGetDeviceProcAddr get = (PFN_vkGetDeviceProcAddr)instance_get(n->instance, "vkGetDeviceProcAddr");
    CHECK(get); c->reset_pool = (PFN_vkResetCommandPool)get(n->device, "vkResetCommandPool"); CHECK(c->reset_pool);
    CHECK(native_compute(n, &c->program, transform_code, sizeof(transform_code), sizeof(TransformArguments), transform_local));
    uint32_t count = 0; n->vkGetPhysicalDeviceQueueFamilyProperties(n->physical, &count, NULL);
    CHECK(count <= 128); VkQueueFamilyProperties families[128];
    n->vkGetPhysicalDeviceQueueFamilyProperties(n->physical, &count, families);
    for (unsigned i = 0; i < count; ++i) printf("QUEUE {\"family\":%u,\"count\":%u,\"flags\":%u}\n", i, families[i].queueCount, families[i].queueFlags);
#else
    API(ogpu_probe_create(OGPU_ABI_VERSION, &c->probe, &c->error));
    uint32_t count = 0; API(ogpu_probe_device_count(c->probe, &count)); CHECK(count == 1);
    API(ogpu_device_create_graphics(c->probe, 0, &c->device, &c->error));
    OgpuDeviceInfo info; API(ogpu_probe_device_info(c->probe, 0, &info));
    printf("DEVICE {\"vendor\":%u,\"device\":%u,\"api\":[%u,%u,%u]}\n", info.vendor_id, info.device_id,
        info.vulkan_api_major, info.vulkan_api_minor, info.vulkan_api_patch);
    OgpuCapabilities capabilities; API(ogpu_device_capabilities(c->device, &capabilities, &c->error));
    OgpuDeviceLimits limits; API(ogpu_device_limits(c->device, &limits, &c->error));
    CHECK(transform_compatible(&capabilities, &limits)); OgpuShaderDesc shader = transform_shader();
    API(ogpu_kernel_create(c->device, &shader, sizeof(TransformArguments), &c->kernel, &c->error));
#endif
    for (unsigned j = 0; j < c->count; ++j) {
        Slot *s = &c->slots[j]; uint32_t data[WORDS]; uint64_t address;
#ifdef FRONTIER_NATIVE
        CHECK(native_buffer_create(n, &s->buffer, sizeof(data), 1)); address = s->buffer.address;
#else
        API(ogpu_buffer_create(c->device, sizeof(data), OGPU_MEMORY_HOST, &s->buffer, &c->error));
        API(ogpu_buffer_device_address(s->buffer, &address, &c->error));
#endif
        for (unsigned i = 0; i < WORDS; ++i) data[i] = i < GUARD_WORDS || i >= GUARD_WORDS + ELEMENTS
            ? sentinel : initial(j, i - GUARD_WORDS);
        CHECK(write_slot(c, s, data));
        s->root = (TransformArguments){.arg_data=address + GUARD_WORDS * 4, .arg_count=ELEMENTS};
#ifdef FRONTIER_NATIVE
        if (!strcmp(c->policy, "replay")) CHECK(record_slot(c, s));
#endif
    }
    return 1;
}
static int retire(Context *c, unsigned slot, Frame *frames, int validate) {
    Slot *s = &c->slots[slot];
    if (!s->pending) return 1;
    Frame *f = &frames[s->sample_index]; double start = clock_ms();
    CHECK(wait_slot(c, s)); double end = clock_ms();
    f->wait = end - start; f->latency = end - f->start;
    if (validate) CHECK(verify(c, slot));
    return 1;
}
static int window(Context *c, unsigned count, Frame *frames, int validate, double *wall) {
    double start = clock_ms();
    for (unsigned i = 0; i < count; ++i) {
        unsigned slot = i % c->count; Slot *s = &c->slots[slot];
        CHECK(retire(c, slot, frames, validate));
        s->sample_index = i; frames[i].start = clock_ms();
        CHECK(record_slot(c, s)); double recorded = clock_ms();
        CHECK(submit_slot(c, s)); double submitted = clock_ms();
        frames[i].record = recorded - frames[i].start; frames[i].submit = submitted - recorded;
    }
    /* Drain in submission order, not slot-index order after wraparound. */
    unsigned first = count > c->count ? count - c->count : 0;
    for (unsigned i = first; i < count; ++i) CHECK(retire(c, i % c->count, frames, validate));
    *wall = clock_ms() - start;
    for (unsigned i = 0; i < c->count; ++i) CHECK(verify(c, i));
    return 1;
}
static int selftest(void) {
    CHECK(number("1000") == 1000 && !number("-1") && !number("1x") && !number("10001"));
    for (unsigned seed = 0; seed < 5; ++seed) {
        uint32_t x = initial(seed, 64);
        for (unsigned i = 0; i < 1000; ++i) { CHECK(expected(initial(seed, 64), i) == x); x = x * 3u + 7u; }
    }
    puts("Frontier parsing/oracle selftest PASS"); return 1;
}
#ifndef FRONTIER_NO_MAIN
int main(int argc, char **argv) {
    if (argc == 2 && !strcmp(argv[1], "--selftest")) return !selftest();
    if (argc != 6) { fprintf(stderr, "Usage: small policy slots dispatches frames validate|measure\n"); return 1; }
    Context c = {0}; c.policy = argv[1]; c.count = number(argv[2]); c.dispatches = number(argv[3]);
    unsigned count = number(argv[4]); int validate = !strcmp(argv[5], "validate");
    if ((c.count != 1 && c.count != 3) || (c.dispatches != 1 && c.dispatches != 64) || !count
        || (!validate && strcmp(argv[5], "measure"))) return 1;
#ifdef FRONTIER_NATIVE
    if (strcmp(c.policy, "fresh") && strcmp(c.policy, "reset") && strcmp(c.policy, "replay")) return 1;
#else
    if (strcmp(c.policy, "ogpu")) return 1;
#endif
    Frame *frames = calloc(count > 100 ? count : 100, sizeof(*frames));
    if (!frames) return 1;
    int okay = 0; double setup = clock_ms(), wall = 0;
    if (!create(&c)) goto cleanup;
    setup = clock_ms() - setup;
    if (!validate && !window(&c, 100, frames, 0, &wall)) goto cleanup;
    memset(frames, 0, (count > 100 ? count : 100) * sizeof(*frames));
    if (!window(&c, count, frames, validate, &wall)) goto cleanup;
    printf("RESULT {\"policy\":\"%s\",\"slots\":%u,\"dispatches\":%u,\"frames\":%u,"
        "\"warmups\":%u,\"validation\":%s,\"setup_ms\":%.9f,\"wall_ms\":%.9f,\"requested_bytes\":%u}\n",
        c.policy, c.count, c.dispatches, count, validate ? 0 : 100, validate ? "true" : "false", setup, wall, c.count * WORDS * 4);
    if (!validate) for (unsigned i = 0; i < count; ++i) printf("FRAME {\"index\":%u,\"record_ms\":%.9f,"
        "\"submit_ms\":%.9f,\"wait_ms\":%.9f,\"latency_ms\":%.9f}\n",
        i, frames[i].record, frames[i].submit, frames[i].wait, frames[i].latency);
    okay = 1;
cleanup:
    destroy(&c); free(frames);
    if (okay) puts("Frontier full-output/guards PASS; all slots drained");
    return !okay;
}
#endif
