/* Bounded D4 experiment: three reusable scratch ranges, twelve independent jobs.
 * Run both application-owned and explicitly retained allocation modes. */
#include "ogpu.h"
#include <inttypes.h>
#include <sched.h>
#include <stdio.h>
#include <stdlib.h>

#define REQUIRE(x) do { if (!(x)) { fprintf(stderr, "Check failed line %d: %s\n", __LINE__, #x); goto cleanup; } } while (0)
#define TRY(x) do { OgpuResult s_ = (x); if (s_ != OGPU_SUCCESS) { fprintf(stderr, "%s: %s\n", #x, error.message); goto cleanup; } } while (0)
enum { SLOTS = 3, JOBS = 12, COUNT = 65, STRIDE = COUNT + 2 };
typedef struct Root { uint64_t input, output; uint32_t count, reserved; } Root;
_Static_assert(sizeof(Root) == 24 && offsetof(Root, count) == 16, "shader root ABI");

static int kernel(OgpuDevice *device, const char *path, OgpuKernel **out) {
    int result = EXIT_FAILURE;
    OgpuError error = {0};
    uint32_t *words = NULL;
    FILE *file = fopen(path, "rb");
    REQUIRE(file && fseek(file, 0, SEEK_END) == 0);
    long size = ftell(file);
    REQUIRE(size >= 20 && size % 4 == 0 && fseek(file, 0, SEEK_SET) == 0);
    words = malloc((size_t)size);
    REQUIRE(words && fread(words, 1, (size_t)size, file) == (size_t)size);
    TRY(ogpu_kernel_create(device, words, (uint64_t)size / 4, sizeof(Root), out, &error));
    result = EXIT_SUCCESS;
cleanup:
    free(words);
    if (file) fclose(file);
    return result;
}

static int run(OgpuDevice *device, OgpuKernel *produce, OgpuKernel *consume, int assisted) {
    int result = EXIT_FAILURE;
    OgpuError error = {0};
    OgpuBuffer *input = NULL, *scratch = NULL, *output = NULL;
    OgpuBatch *batch = NULL;
    OgpuCompletion *slots[SLOTS] = {0};
    uint32_t inputs[JOBS * COUNT], outputs[JOBS * STRIDE];
    const uint32_t guard = 0xa5a5a5a5u;
    uint64_t input_address, scratch_address, output_address;
    unsigned reused = 0, with_other_handles = 0;
    for (unsigned i = 0; i < JOBS * COUNT; ++i) inputs[i] = i * 13u + 5u;
    for (unsigned i = 0; i < JOBS * STRIDE; ++i) outputs[i] = guard;
    TRY(ogpu_buffer_create(device, sizeof(inputs), OGPU_MEMORY_HOST, &input, &error));
    TRY(ogpu_buffer_create(device, SLOTS * COUNT * 4, OGPU_MEMORY_HOST, &scratch, &error));
    TRY(ogpu_buffer_create(device, sizeof(outputs), OGPU_MEMORY_HOST, &output, &error));
    TRY(ogpu_buffer_write(input, 0, inputs, sizeof(inputs), &error));
    TRY(ogpu_buffer_write(output, 0, outputs, sizeof(outputs), &error));
    TRY(ogpu_buffer_device_address(input, &input_address, &error));
    TRY(ogpu_buffer_device_address(scratch, &scratch_address, &error));
    TRY(ogpu_buffer_device_address(output, &output_address, &error));
    for (unsigned job = 0; job < JOBS; ++job) {
        unsigned slot = job % SLOTS;
        if (slots[slot]) {
            uint32_t complete = 0;
            // Bounded demo polling; exhaustion is a test failure, not permission to reuse.
            for (unsigned tries = 0; !complete && tries < 1000000; ++tries) {
                TRY(ogpu_completion_poll(slots[slot], &complete, &error));
                if (!complete) sched_yield();
            }
            REQUIRE(complete);
            ++reused;
            for (unsigned other = 0; other < SLOTS; ++other)
                if (other != slot && slots[other]) { ++with_other_handles; break; }
            ogpu_completion_destroy(slots[slot]);
            slots[slot] = NULL;
        }
        TRY(ogpu_batch_create(device, &batch, &error));
        if (assisted) TRY(ogpu_batch_retain_buffer(batch, scratch, &error));
        // Allocation retention has no synchronization effect. Keep explicit dependencies.
        TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_COMPUTE_WRITE,
            OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_COMPUTE_WRITE, &error));
        Root a = {input_address + job * COUNT * 4, scratch_address + slot * COUNT * 4, COUNT, 0};
        Root b = {a.output, output_address + (job * STRIDE + 1) * 4, COUNT, 0};
        TRY(ogpu_batch_dispatch(batch, produce, (COUNT + 63) / 64, &a, sizeof(a), &error));
        TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_WRITE, OGPU_ACCESS_COMPUTE_READ, &error));
        TRY(ogpu_batch_dispatch(batch, consume, (COUNT + 63) / 64, &b, sizeof(b), &error));
        TRY(ogpu_batch_submit(batch, &slots[slot], &error));
        ogpu_batch_destroy(batch);
        batch = NULL;
    }
    if (assisted) {
        // All final uses explicitly retain scratch; raw addresses alone would not suffice.
        ogpu_buffer_destroy(scratch);
        scratch = NULL;
    }
    for (unsigned i = 0; i < SLOTS; ++i) TRY(ogpu_completion_wait(slots[i], &error));
    // No CPU access to any part of shared output storage until ALL its uses finish.
    TRY(ogpu_buffer_read(output, 0, outputs, sizeof(outputs), &error));
    for (unsigned job = 0; job < JOBS; ++job) {
        REQUIRE(outputs[job * STRIDE] == guard && outputs[job * STRIDE + STRIDE - 1] == guard);
        for (unsigned i = 0; i < COUNT; ++i) {
            uint32_t a = inputs[job * COUNT + i] * 3u + 7u;
            uint32_t b = inputs[job * COUNT + (i + 1) % COUNT] * 3u + 7u;
            REQUIRE(outputs[job * STRIDE + i + 1] == a + b * 5u + 11u);
        }
    }
    REQUIRE(reused == JOBS - SLOTS && with_other_handles == reused);
    printf("retirement mode=%s jobs=%d slots=%d safe_reuses=%u other_live_completions=%u PASS\n",
        assisted ? "retained" : "caller-owned", JOBS, SLOTS, reused, with_other_handles);
    result = EXIT_SUCCESS;
cleanup:
    for (unsigned i = 0; i < SLOTS; ++i) ogpu_completion_destroy(slots[i]);
    ogpu_batch_destroy(batch);
    ogpu_buffer_destroy(output);
    ogpu_buffer_destroy(scratch);
    ogpu_buffer_destroy(input);
    return result;
}

int main(int argc, char **argv) {
    int result = EXIT_FAILURE;
    OgpuError error = {0};
    OgpuProbe *probe = NULL;
    OgpuDevice *device = NULL;
    OgpuKernel *produce = NULL, *consume = NULL;
    REQUIRE(argc == 3);
    TRY(ogpu_probe_create(OGPU_ABI_VERSION, &probe, &error));
    uint32_t count = 0;
    TRY(ogpu_probe_device_count(probe, &count));
    for (uint32_t i = 0; i < count; ++i) {
        OgpuResult status = ogpu_device_create(probe, i, &device, &error);
        if (status == OGPU_ERROR_UNSUPPORTED) continue;
        TRY(status);
        break;
    }
    REQUIRE(device);
    REQUIRE(kernel(device, argv[1], &produce) == EXIT_SUCCESS);
    REQUIRE(kernel(device, argv[2], &consume) == EXIT_SUCCESS);
    REQUIRE(run(device, produce, consume, 0) == EXIT_SUCCESS);
    REQUIRE(run(device, produce, consume, 1) == EXIT_SUCCESS);
    result = EXIT_SUCCESS;
cleanup:
    ogpu_kernel_destroy(consume);
    ogpu_kernel_destroy(produce);
    ogpu_device_destroy(device);
    ogpu_probe_destroy(probe);
    return result;
}
