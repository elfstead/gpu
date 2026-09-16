#define _POSIX_C_SOURCE 200809L
#include "ogpu.h"
#include <errno.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define REQUIRE(x) do { if (!(x)) { fprintf(stderr, "Check failed line %d: %s\n", __LINE__, #x); goto cleanup; } } while (0)
#define TRY(x) do { OgpuResult s_ = (x); if (s_ != OGPU_SUCCESS) { fprintf(stderr, "%s: %d %s\n", #x, s_, error.message); goto cleanup; } } while (0)
enum { INPUT, WEIGHTS, HIDDEN, DENOISED, COLOR, BUFFER_COUNT, GUARD = 64 };
typedef struct HiddenRoot { uint64_t input, weights, output; uint32_t width, height; } HiddenRoot;
typedef struct DenoiseRoot { uint64_t input, weights, hidden, output; uint32_t count, reserved; } DenoiseRoot;
typedef struct ProcessRoot { uint64_t input, output; uint32_t width, height, out_width, out_height; } ProcessRoot;
typedef struct ReadRoot { uint64_t pixels; uint32_t width, reserved; } ReadRoot;
typedef struct PoisonRoot { uint64_t output; uint32_t count, reserved; } PoisonRoot;
_Static_assert(sizeof(HiddenRoot) == 32 && offsetof(HiddenRoot, width) == 24, "hidden root");
_Static_assert(sizeof(DenoiseRoot) == 40 && offsetof(DenoiseRoot, count) == 32, "denoise root");
_Static_assert(sizeof(ProcessRoot) == 32 && offsetof(ProcessRoot, width) == 16, "process root");
_Static_assert(sizeof(ReadRoot) == 16 && offsetof(ReadRoot, width) == 8, "read root");
_Static_assert(sizeof(PoisonRoot) == 16 && offsetof(PoisonRoot, count) == 8, "poison root");

static double now_ms(void) {
    struct timespec time;
    if (clock_gettime(CLOCK_MONOTONIC, &time)) abort();
    return (double)time.tv_sec * 1000.0 + (double)time.tv_nsec / 1e6;
}

static int path_join(char path[4096], const char *directory, const char *file) {
    int size = snprintf(path, 4096, "%s/%s", directory, file);
    return size >= 0 && size < 4096;
}

static int read_exact(const char *path, void *data, size_t size) {
    FILE *file = fopen(path, "rb");
    if (!file) { perror(path); return 0; }
    int okay = fread(data, 1, size, file) == size && fgetc(file) == EOF && !ferror(file);
    if (fclose(file)) okay = 0;
    return okay;
}

static int write_exact(const char *path, const void *data, size_t size) {
    FILE *file = fopen(path, "wb");
    if (!file) { perror(path); return 0; }
    int okay = fwrite(data, 1, size, file) == size;
    if (fclose(file)) okay = 0;
    return okay;
}

static int read_shader(const char *directory, const char *name, OgpuShaderDesc *shader) {
    char path[4096];
    if (!path_join(path, directory, name)) return 0;
    FILE *file = fopen(path, "rb");
    if (!file) { perror(path); return 0; }
    int okay = 0;
    void *data = NULL;
    if (fseek(file, 0, SEEK_END)) goto cleanup;
    long size = ftell(file);
    if (size < 20 || size % 4 || fseek(file, 0, SEEK_SET)) goto cleanup;
    data = malloc((size_t)size);
    if (!data || fread(data, 1, (size_t)size, file) != (size_t)size) goto cleanup;
    *shader = (OgpuShaderDesc){.code = data, .code_size = (uint64_t)size, .format = OGPU_SHADER_SPIRV};
    data = NULL;
    okay = 1;
cleanup:
    free(data);
    fclose(file);
    return okay;
}

static uint32_t extent(const char *text) {
    char *end;
    errno = 0;
    unsigned long value = strtoul(text, &end, 10);
    return errno || !*text || *end || value == 0 || value > 1024 ? 0 : (uint32_t)value;
}

static void poison_host(void *data, size_t bytes) {
    const uint32_t word = UINT32_C(0x7fc000a5);
    for (size_t i = 0; i < bytes; i += 4) memcpy((char *)data + i, &word, 4);
}

static int guards(const unsigned char *data, size_t bytes) {
    for (unsigned i = 0; i < GUARD; i += 4) {
        uint32_t a, b;
        memcpy(&a, data + i, 4); memcpy(&b, data + bytes - GUARD + i, 4);
        if (a != UINT32_C(0x7fc000a5) || b != UINT32_C(0x7fc000a5)) return 0;
    }
    return 1;
}

int main(int argc, char **argv) {
    int result = EXIT_FAILURE;
    OgpuError error = {0};
    OgpuProbe *probe = NULL;
    OgpuDevice *device = NULL;
    OgpuBuffer *buffers[BUFFER_COUNT] = {0}, *upload = NULL, *readback = NULL, *draw = NULL;
    OgpuKernel *kernels[4] = {0};
    OgpuRaster *raster = NULL;
    OgpuImage *target = NULL;
    OgpuBatch *batch = NULL;
    OgpuCompletion *done = NULL;
    OgpuShaderDesc shaders[6] = {0};
    unsigned char *host = NULL, *weights = NULL, *pixels = NULL;
    char path[4096], name[96];
    double setup_start = now_ms();
    // All files are trusted, generated little-endian fixtures, not a model format.
    REQUIRE(argc >= 9);
    const uint16_t endian = 1;
    REQUIRE(*(const unsigned char *)&endian == 1);
    uint32_t width = extent(argv[3]), height = extent(argv[4]);
    uint32_t ow = extent(argv[5]), oh = extent(argv[6]);
    REQUIRE(width && height && ow && oh);
    uint32_t count = width * height, output_count = ow * oh;
    size_t payloads[BUFFER_COUNT] = {count * 4u, 89u * 4u, count * 32u, count * 4u, output_count * 16u};
    size_t sizes[BUFFER_COUNT], offsets[BUFFER_COUNT], total = output_count * 4u + 2 * GUARD;
    uint64_t addresses[BUFFER_COUNT] = {0};
    for (unsigned i = 0; i < BUFFER_COUNT; ++i) {
        sizes[i] = payloads[i] + 2 * GUARD;
        offsets[i] = total;
        total += sizes[i];
    }
    size_t upload_size = sizes[INPUT] > sizes[WEIGHTS] ? sizes[INPUT] : sizes[WEIGHTS];
    host = malloc(upload_size); weights = malloc(sizes[WEIGHTS]); pixels = malloc(total);
    REQUIRE(host && weights && pixels);
    poison_host(weights, sizes[WEIGHTS]);
    REQUIRE(read_exact(argv[2], weights + GUARD, payloads[WEIGHTS]));
    TRY(ogpu_probe_create(OGPU_ABI_VERSION, &probe, &error));
    uint32_t devices = 0;
    TRY(ogpu_probe_device_count(probe, &devices));
    for (uint32_t i = 0; i < devices; ++i) {
        OgpuResult status = ogpu_device_create_graphics(probe, i, &device, &error);
        if (status == OGPU_ERROR_UNSUPPORTED) continue;
        TRY(status);
        OgpuDeviceInfo info;
        TRY(ogpu_probe_device_info(probe, i, &info));
        printf("Learned-image device: %s (backend=%u)\n", info.name, info.backend);
        break;
    }
    REQUIRE(device);
    OgpuDeviceLimits limits;
    TRY(ogpu_device_limits(device, &limits, &error));
    REQUIRE(limits.max_push_data_bytes >= sizeof(DenoiseRoot) && limits.max_group_invocations >= 64
        && limits.max_group_size[0] >= 64 && limits.max_group_size[1] >= 1 && limits.max_group_size[2] >= 1);
    REQUIRE(ow <= limits.max_image_2d && oh <= limits.max_image_2d);
    REQUIRE((count * 8u + 32u + 63u) / 64u <= limits.max_dispatch[0]);
    REQUIRE((output_count * 4u + 32u + 63u) / 64u <= limits.max_dispatch[0]);
    OgpuTimingInfo timing;
    OgpuResult timing_status = ogpu_device_timing_info(device, &timing, &error);
    REQUIRE(timing_status == OGPU_SUCCESS || timing_status == OGPU_ERROR_UNSUPPORTED);
    int timed = timing_status == OGPU_SUCCESS;
    const char *shader_names[] = {"hidden.comp.spv", "denoise.comp.spv", "process.comp.spv",
        "poison.comp.spv", "fullscreen.vert.spv", "display.frag.spv"};
    const uint32_t roots[] = {sizeof(HiddenRoot), sizeof(DenoiseRoot), sizeof(ProcessRoot), sizeof(PoisonRoot)};
    for (unsigned i = 0; i < 6; ++i) REQUIRE(read_shader(argv[1], shader_names[i], &shaders[i]));
    for (unsigned i = 0; i < 4; ++i) TRY(ogpu_kernel_create(device, &shaders[i], roots[i], &kernels[i], &error));
    TRY(ogpu_raster_create(device, &shaders[4], &shaders[5], sizeof(ReadRoot),
        OGPU_TOPOLOGY_TRIANGLE_LIST, OGPU_FORMAT_RGBA8_UNORM, &raster, &error));
    for (unsigned i = 0; i < BUFFER_COUNT; ++i) {
        TRY(ogpu_buffer_create(device, sizes[i], OGPU_MEMORY_DEVICE, &buffers[i], &error));
        TRY(ogpu_buffer_device_address(buffers[i], &addresses[i], &error));
        addresses[i] += GUARD;
    }
    TRY(ogpu_buffer_create(device, upload_size, OGPU_MEMORY_HOST, &upload, &error));
    TRY(ogpu_buffer_create(device, total, OGPU_MEMORY_HOST, &readback, &error));
    TRY(ogpu_buffer_create(device, sizeof(OgpuDrawArguments), OGPU_MEMORY_HOST, &draw, &error));
    const OgpuDrawArguments draw_args = {3, 1, 0, 0};
    TRY(ogpu_buffer_write(draw, 0, &draw_args, sizeof(draw_args), &error));
    OgpuImageDesc image = {OGPU_IMAGE_2D, ow, oh, OGPU_FORMAT_RGBA8_UNORM,
        OGPU_IMAGE_USAGE_COLOR | OGPU_IMAGE_USAGE_COPY_SRC, 0};
    TRY(ogpu_image_create(device, &image, &target, &error));
    // Setup only: upload guarded weights and initialize scratch guards on GPU.
    TRY(ogpu_buffer_write(upload, 0, weights, sizes[WEIGHTS], &error));
    TRY(ogpu_batch_create(device, &batch, &error));
    TRY(ogpu_batch_copy_buffer(batch, upload, 0, buffers[WEIGHTS], 0, sizes[WEIGHTS], &error));
    for (unsigned i = HIDDEN; i < BUFFER_COUNT; ++i) {
        PoisonRoot root = {addresses[i] - GUARD, (uint32_t)(sizes[i] / 4), 0};
        TRY(ogpu_batch_dispatch(batch, kernels[3], (root.count + 63) / 64, 1, 1, &root, sizeof(root), &error));
    }
    TRY(ogpu_batch_submit(batch, &done, &error));
    TRY(ogpu_completion_wait(done, &error));
    ogpu_completion_destroy(done); done = NULL;
    ogpu_batch_destroy(batch); batch = NULL;
    printf("Setup: %.3f ms; weights upload=%zu payload bytes (guards +%u); no per-frame model upload\n",
        now_ms() - setup_start, payloads[WEIGHTS], 2 * GUARD);

    HiddenRoot hidden = {addresses[INPUT], addresses[WEIGHTS], addresses[HIDDEN], width, height};
    DenoiseRoot denoise = {addresses[INPUT], addresses[WEIGHTS], addresses[HIDDEN], addresses[DENOISED], count, 0};
    ProcessRoot process = {addresses[DENOISED], addresses[COLOR], width, height, ow, oh};
    ReadRoot display = {addresses[COLOR], ow, 0};
    for (unsigned diagnostic = 0; diagnostic < 2; ++diagnostic) {
        for (int frame = 8; frame < argc; ++frame) {
            REQUIRE(path_join(path, argv[frame], "input.f32"));
            poison_host(host, sizes[INPUT]);
            REQUIRE(read_exact(path, host + GUARD, payloads[INPUT]));
            // This is before submission, never a mid-pipeline CPU operation.
            double host_write_start = now_ms();
            TRY(ogpu_buffer_write(upload, 0, host, sizes[INPUT], &error));
            size_t read_size = diagnostic ? total : output_count * 4u + 2 * GUARD;
            poison_host(pixels, read_size);
            TRY(ogpu_buffer_write(readback, 0, pixels, read_size, &error));
            double host_write_ms = now_ms() - host_write_start;
            double execute_start = now_ms();
            TRY(ogpu_batch_create(device, &batch, &error));
            if (timed) TRY(ogpu_batch_enable_timing(batch, &error));
            // Completion allows CPU reuse; explicit dependencies order GPU reuse.
            TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_COMPUTE_WRITE
                | OGPU_ACCESS_FRAGMENT_READ | OGPU_ACCESS_TRANSFER_READ | OGPU_ACCESS_TRANSFER_WRITE,
                OGPU_ACCESS_COMPUTE_WRITE | OGPU_ACCESS_COMPUTE_READ | OGPU_ACCESS_TRANSFER_WRITE, &error));
            TRY(ogpu_batch_copy_buffer(batch, upload, 0, buffers[INPUT], 0, sizes[INPUT], &error));
            TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_TRANSFER_WRITE, OGPU_ACCESS_COMPUTE_READ, &error));
            if (diagnostic) {
                for (unsigned i = HIDDEN; i < BUFFER_COUNT; ++i) {
                    PoisonRoot root = {addresses[i] - GUARD, (uint32_t)(sizes[i] / 4), 0};
                    TRY(ogpu_batch_dispatch(batch, kernels[3], (root.count + 63) / 64, 1, 1, &root, sizeof(root), &error));
                }
                TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_WRITE, OGPU_ACCESS_COMPUTE_WRITE, &error));
            }
            TRY(ogpu_batch_dispatch(batch, kernels[0], (count * 8u + 63u) / 64u, 1, 1, &hidden, sizeof(hidden), &error));
            TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_WRITE, OGPU_ACCESS_COMPUTE_READ, &error));
            TRY(ogpu_batch_dispatch(batch, kernels[1], (count + 63u) / 64u, 1, 1, &denoise, sizeof(denoise), &error));
            TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_WRITE, OGPU_ACCESS_COMPUTE_READ, &error));
            TRY(ogpu_batch_dispatch(batch, kernels[2], (output_count + 63u) / 64u, 1, 1, &process, sizeof(process), &error));
            TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_WRITE, OGPU_ACCESS_FRAGMENT_READ, &error));
            // The fragment shader reads the compute-written allocation directly.
            // No intermediate buffer->image representation copy is necessary.
            TRY(ogpu_batch_draw_indirect(batch, raster, target, draw, 0, &display, sizeof(display), OGPU_ATTACHMENT_CLEAR, &error));
            TRY(ogpu_batch_copy_image_to_buffer(batch, target, readback, GUARD, &error));
            if (diagnostic) {
                TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_WRITE | OGPU_ACCESS_TRANSFER_WRITE,
                    OGPU_ACCESS_TRANSFER_READ, &error));
                for (unsigned i = 0; i < BUFFER_COUNT; ++i)
                    TRY(ogpu_batch_copy_buffer(batch, buffers[i], 0, readback, offsets[i], sizes[i], &error));
            }
            // Public owners remain alive through the wait. Pointer values alone
            // do not keep input/weights/activations alive; there is no graph owner.
            TRY(ogpu_batch_submit(batch, &done, &error));
            ogpu_batch_destroy(batch); batch = NULL;
            TRY(ogpu_completion_wait(done, &error));
            double execute_ms = now_ms() - execute_start;
            double query_start = now_ms(), device_ns = 0;
            if (timed) TRY(ogpu_completion_elapsed_ns(done, &device_ns, &error));
            double query_ms = now_ms() - query_start;
            ogpu_completion_destroy(done); done = NULL;
            double read_start = now_ms();
            TRY(ogpu_buffer_read(readback, 0, pixels, read_size, &error));
            double read_ms = now_ms() - read_start;
            REQUIRE(guards(pixels, output_count * 4u + 2 * GUARD));
            snprintf(name, sizeof(name), "%s-%d-final.rgba", diagnostic ? "diagnostic" : "normal", frame - 8);
            REQUIRE(path_join(path, argv[7], name));
            REQUIRE(write_exact(path, pixels + GUARD, output_count * 4u));
            if (diagnostic) {
                const char *names[] = {"input", "weights", "hidden", "denoised", "processed"};
                for (unsigned i = 0; i < BUFFER_COUNT; ++i) {
                    REQUIRE(guards(pixels + offsets[i], sizes[i]));
                    snprintf(name, sizeof(name), "diagnostic-%d-%s.f32", frame - 8, names[i]);
                    REQUIRE(path_join(path, argv[7], name));
                    REQUIRE(write_exact(path, pixels + offsets[i] + GUARD, payloads[i]));
                }
                REQUIRE(memcmp(pixels + offsets[INPUT], host, sizes[INPUT]) == 0);
                REQUIRE(memcmp(pixels + offsets[WEIGHTS], weights, sizes[WEIGHTS]) == 0);
            }
            printf("%s frame %d %ux%u -> %ux%u: host_write=%.3f host_execute=%.3f read=%.3f query=%.3f ms; ",
                diagnostic ? "diagnostic" : "normal", frame - 8, width, height, ow, oh,
                host_write_ms, execute_ms, read_ms, query_ms);
            if (timed) printf("device_batch=%.3f ms", device_ns / 1e6);
            else printf("device_batch=unsupported");
            printf("; GPU upload=%zu final_copy=%u diagnostic_copy=%zu bytes; guards PASS\n",
                sizes[INPUT], output_count * 4u, diagnostic ? total - output_count * 4u - 2 * GUARD : 0);
        }
    }
    result = EXIT_SUCCESS;
cleanup:
    // Drain pending work before releasing any pointer-reachable buffers on error.
    ogpu_completion_destroy(done);
    ogpu_batch_destroy(batch);
    ogpu_image_destroy(target);
    ogpu_raster_destroy(raster);
    for (unsigned i = 0; i < 4; ++i) ogpu_kernel_destroy(kernels[i]);
    for (unsigned i = 0; i < BUFFER_COUNT; ++i) ogpu_buffer_destroy(buffers[i]);
    ogpu_buffer_destroy(draw); ogpu_buffer_destroy(readback); ogpu_buffer_destroy(upload);
    ogpu_device_destroy(device); ogpu_probe_destroy(probe);
    for (unsigned i = 0; i < 6; ++i) free((void *)shaders[i].code);
    free(host); free(weights); free(pixels);
    return result;
}
