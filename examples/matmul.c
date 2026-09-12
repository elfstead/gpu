#define _POSIX_C_SOURCE 200809L
#include "ogpu.h"
#include <float.h>
#include <inttypes.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define REQUIRE(condition) do { if (!(condition)) { \
    fprintf(stderr, "Check failed at line %d: %s\n", __LINE__, #condition); goto cleanup; \
} } while (0)
#define TRY(operation) do { OgpuResult status_ = (operation); if (status_ != OGPU_SUCCESS) { \
    fprintf(stderr, "%s: status %" PRId32 ", %s\n", #operation, status_, error.message); goto cleanup; \
} } while (0)

enum { WARMUPS = 2, SAMPLES = 9 };
static const uint32_t poison = 0x7fc0a5a5u;
static const char *names[2] = {"naive", "tiled8"};
typedef struct Root {
    uint64_t a, b, c;
    uint32_t m, n, k, lda, ldb, ldc;
} Root;
_Static_assert(sizeof(float) == 4 && FLT_RADIX == 2 && FLT_MANT_DIG == 24, "FP32 host required");
_Static_assert(sizeof(Root) == 48 && offsetof(Root, b) == 8 && offsetof(Root, c) == 16
    && offsetof(Root, m) == 24 && offsetof(Root, n) == 28 && offsetof(Root, k) == 32
    && offsetof(Root, lda) == 36 && offsetof(Root, ldb) == 40 && offsetof(Root, ldc) == 44, "root ABI");

typedef struct Matrix {
    OgpuBuffer *buffer;
    float *host;
    size_t count;
    uint32_t rows, cols, stride;
} Matrix;

static double now_ms(void) {
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t) != 0) { perror("clock_gettime"); exit(EXIT_FAILURE); }
    return (double)t.tv_sec * 1000.0 + (double)t.tv_nsec / 1e6;
}

static double absolute(double x) { return x < 0 ? -x : x; }

static void poison_matrix(Matrix *matrix) {
    for (size_t i = 0; i < matrix->count; ++i) memcpy(&matrix->host[i], &poison, 4);
}

/* Sizes in this experiment are fixed and small enough for uint32 shader indexing. */
static int create_matrix(OgpuDevice *device, uint32_t rows, uint32_t cols,
    uint32_t padding, Matrix *matrix) {
    int exit_code = EXIT_FAILURE;
    OgpuError error = {0};
    matrix->rows = rows; matrix->cols = cols; matrix->stride = cols + padding;
    matrix->count = (size_t)rows * matrix->stride + 2;
    matrix->host = malloc(matrix->count * sizeof(float));
    REQUIRE(matrix->host != NULL);
    TRY(ogpu_buffer_create(device, matrix->count * sizeof(float), &matrix->buffer, &error));
    poison_matrix(matrix);
    exit_code = EXIT_SUCCESS;
cleanup:
    return exit_code; /* Caller owns even partially created matrices. */
}

static float input_value(unsigned pattern, unsigned which, uint32_t row, uint32_t col) {
    const int signed_value = (int)((row * 17u + col * 13u + which * 7u) % 23u) - 11;
    switch (pattern) {
        case 0: return 0.0f;
        case 1: return row == col ? 1.0f : 0.0f;
        case 2: return (float)signed_value / 7.0f;
        case 3: /* Alternating nearly cancelling products along K. */
            return which == 0 ? (col % 2 ? -1.0f : 1.0f)
                : 1.0f + (float)((row + col) % 3u) * 0.0001f;
        default: {
            const float scales[3] = {0.001f, 1.0f, 1000.0f};
            return (float)signed_value / 11.0f * scales[(row + col + which) % 3u];
        }
    }
}

static int execute(OgpuDevice *device, OgpuKernel *kernel, uint32_t groups,
    const Root *root, int timed, double *elapsed, double *device_ms, double *query_ms) {
    int exit_code = EXIT_FAILURE;
    OgpuError error = {0};
    OgpuBatch *batch = NULL;
    OgpuCompletion *completion = NULL;
    const double start = now_ms();
    double wait_end = start;
    double cleanup_start = 0;
    *query_ms = 0;
    TRY(ogpu_batch_create(device, &batch, &error));
    if (timed) TRY(ogpu_batch_enable_timing(batch, &error));
    TRY(ogpu_batch_barrier(batch, OGPU_ACCESS_COMPUTE_WRITE, OGPU_ACCESS_COMPUTE_WRITE, &error));
    TRY(ogpu_batch_dispatch(batch, kernel, groups, root, sizeof(*root), &error));
    TRY(ogpu_batch_submit(batch, &completion, &error));
    TRY(ogpu_completion_wait(completion, &error));
    wait_end = now_ms();
    if (timed) {
        double nanoseconds = 0;
        const double query_start = now_ms();
        TRY(ogpu_completion_elapsed_ns(completion, &nanoseconds, &error));
        *query_ms = now_ms() - query_start;
        REQUIRE(isfinite(nanoseconds) && nanoseconds >= 0);
        *device_ms = nanoseconds / 1e6;
    }
    exit_code = EXIT_SUCCESS;
cleanup:
    cleanup_start = now_ms();
    ogpu_completion_destroy(completion);
    ogpu_batch_destroy(batch);
    /* Retrieval is reported separately, not charged to host execution latency. */
    *elapsed = (wait_end - start) + (now_ms() - cleanup_start);
    return exit_code;
}

static int check_output(const Matrix *c, const double *reference, const double *magnitudes,
    uint32_t k, double *max_absolute, double *max_scaled) {
    for (size_t i = 0; i < c->count; ++i) {
        const size_t offset = i == 0 ? 0 : i - 1;
        const size_t row = offset / c->stride, col = offset % c->stride;
        if (i == 0 || i == c->count - 1 || col >= c->cols) {
            uint32_t bits;
            memcpy(&bits, &c->host[i], 4);
            if (bits != poison) { fprintf(stderr, "Output guard/padding changed at %zu\n", i); return 0; }
        } else {
            const size_t index = row * c->cols + col;
            const double error = absolute((double)c->host[i] - reference[index]);
            const double tolerance = 1e-6 + 8.0 * (k ? k : 1) * FLT_EPSILON * magnitudes[index];
            if (!isfinite(c->host[i]) || error > tolerance) {
                fprintf(stderr, "C[%zu,%zu]: got %.9g, reference %.17g, error %.4g > bound %.4g\n",
                    row, col, (double)c->host[i], reference[index], error, tolerance);
                return 0;
            }
            if (error > *max_absolute) *max_absolute = error;
            if (error / tolerance > *max_scaled) *max_scaled = error / tolerance;
        }
    }
    return 1;
}

static int compare_double(const void *a, const void *b) {
    const double x = *(const double *)a, y = *(const double *)b;
    return (x > y) - (x < y);
}

static void print_samples(const char *label, double samples[SAMPLES]) {
    qsort(samples, SAMPLES, sizeof(double), compare_double);
    printf(" %s %.4f [%.4f, %.4f] ms", label, samples[SAMPLES / 2], samples[0], samples[SAMPLES - 1]);
}

static int run_case(OgpuDevice *device, OgpuKernel *kernels[2], uint32_t m, uint32_t n,
    uint32_t k, unsigned pattern, int benchmark, double wrap_ns, double max_error[2], double max_scaled[2]) {
    int exit_code = EXIT_FAILURE;
    OgpuError error = {0};
    Matrix matrices[3] = {0};
    double *reference = NULL, *magnitudes = NULL;
    float *input_check = NULL;
    double execution[2][2][SAMPLES] = {{{0}}}, readback[2][2][SAMPLES] = {{{0}}}, reset[2][2][SAMPLES] = {{{0}}};
    double device_times[2][SAMPLES] = {{0}}, query_times[2][SAMPLES] = {{0}};
    const unsigned modes = wrap_ns > 0 ? 2 : 1;
    double start = now_ms();
    REQUIRE(create_matrix(device, m, k, 3, &matrices[0]) == EXIT_SUCCESS);
    REQUIRE(create_matrix(device, k, n, 5, &matrices[1]) == EXIT_SUCCESS);
    REQUIRE(create_matrix(device, m, n, 7, &matrices[2]) == EXIT_SUCCESS);
    const double allocation_ms = now_ms() - start;
    for (unsigned which = 0; which < 2; ++which) {
        Matrix *matrix = &matrices[which];
        for (uint32_t row = 0; row < matrix->rows; ++row)
            for (uint32_t col = 0; col < matrix->cols; ++col)
                matrix->host[1 + (size_t)row * matrix->stride + col] = input_value(pattern, which, row, col);
    }
    reference = calloc((size_t)m * n, sizeof(double));
    magnitudes = calloc((size_t)m * n, sizeof(double));
    REQUIRE(reference != NULL && magnitudes != NULL);
    for (uint32_t row = 0; row < m; ++row) {
        for (uint32_t col = 0; col < n; ++col) {
            const size_t i = (size_t)row * n + col;
            for (uint32_t inner = 0; inner < k; ++inner) {
                const double product = (double)matrices[0].host[1 + (size_t)row * matrices[0].stride + inner]
                    * (double)matrices[1].host[1 + (size_t)inner * matrices[1].stride + col];
                reference[i] += product;
                magnitudes[i] += absolute(product);
            }
        }
    }
    start = now_ms();
    for (unsigned i = 0; i < 3; ++i)
        TRY(ogpu_buffer_write(matrices[i].buffer, 0, matrices[i].host, matrices[i].count * 4, &error));
    const double upload_ms = now_ms() - start;
    Root root = {.m = m, .n = n, .k = k, .lda = matrices[0].stride,
        .ldb = matrices[1].stride, .ldc = matrices[2].stride};
    TRY(ogpu_buffer_device_address(matrices[0].buffer, &root.a, &error));
    TRY(ogpu_buffer_device_address(matrices[1].buffer, &root.b, &error));
    TRY(ogpu_buffer_device_address(matrices[2].buffer, &root.c, &error));
    root.a += 4; root.b += 4; root.c += 4;
    const uint32_t groups[2] = {(m * n + 63) / 64, ((m + 7) / 8) * ((n + 7) / 8)};
    Matrix *c = &matrices[2];
    for (unsigned round = 0; round < (benchmark ? WARMUPS + SAMPLES : 1); ++round) {
        for (unsigned order = 0; order < 2; ++order) {
            const unsigned variant = (order + round) % 2;
            for (unsigned mode_order = 0; mode_order < modes; ++mode_order) {
                const unsigned mode = (mode_order + round) % modes;
                start = now_ms();
                poison_matrix(c);
                TRY(ogpu_buffer_write(c->buffer, 0, c->host, c->count * 4, &error));
                const double reset_ms = now_ms() - start;
                double elapsed = 0, gpu_ms = 0, query_ms = 0;
                REQUIRE(execute(device, kernels[variant], groups[variant], &root, mode != 0,
                    &elapsed, &gpu_ms, &query_ms) == EXIT_SUCCESS);
                /* This isolated call's host interval bounds the bracket. Do not silently
                 * accept durations that could contain an undetectable full counter wrap. */
                if (mode) REQUIRE(elapsed * 1e6 < wrap_ns);
                start = now_ms();
                TRY(ogpu_buffer_read(c->buffer, 0, c->host, c->count * 4, &error));
                const double read_ms = now_ms() - start;
                REQUIRE(check_output(c, reference, magnitudes, k, &max_error[variant], &max_scaled[variant]));
                if (benchmark && round >= WARMUPS) {
                    const unsigned sample = round - WARMUPS;
                    execution[mode][variant][sample] = elapsed;
                    readback[mode][variant][sample] = read_ms;
                    reset[mode][variant][sample] = reset_ms;
                    if (mode) {
                        device_times[variant][sample] = gpu_ms;
                        query_times[variant][sample] = query_ms;
                    }
                }
            }
        }
    }
    for (unsigned i = 0; i < 2; ++i) {
        input_check = malloc(matrices[i].count * 4);
        REQUIRE(input_check != NULL);
        TRY(ogpu_buffer_read(matrices[i].buffer, 0, input_check, matrices[i].count * 4, &error));
        REQUIRE(memcmp(input_check, matrices[i].host, matrices[i].count * 4) == 0);
        free(input_check); input_check = NULL;
    }
    if (benchmark) {
        printf("M=%" PRIu32 " N=%" PRIu32 " K=%" PRIu32 ": allocations+host staging %.4f ms, initial upload %.4f ms\n",
            m, n, k, allocation_ms, upload_ms);
        for (unsigned i = 0; i < 2; ++i) {
            for (unsigned mode = 0; mode < modes; ++mode) {
                printf("  %s %s:", names[i], mode ? "timed" : "untimed");
                print_samples("host-execution", execution[mode][i]);
                if (mode) {
                    print_samples("device-batch", device_times[i]);
                    print_samples("query-read", query_times[i]);
                }
                print_samples("output-reset", reset[mode][i]);
                print_samples("readback", readback[mode][i]);
                putchar('\n');
            }
        }
    }
    exit_code = EXIT_SUCCESS;
cleanup:
    free(input_check);
    free(magnitudes);
    free(reference);
    for (unsigned i = 0; i < 3; ++i) {
        ogpu_buffer_destroy(matrices[i].buffer);
        free(matrices[i].host);
    }
    return exit_code;
}

static int read_shader(const char *path, uint32_t **words, uint64_t *count) {
    int exit_code = EXIT_FAILURE;
    FILE *file = fopen(path, "rb");
    REQUIRE(file != NULL);
    REQUIRE(fseek(file, 0, SEEK_END) == 0);
    long bytes = ftell(file);
    REQUIRE(bytes >= 20 && bytes % 4 == 0);
    REQUIRE(fseek(file, 0, SEEK_SET) == 0);
    *words = malloc((size_t)bytes);
    REQUIRE(*words != NULL);
    REQUIRE(fread(*words, 1, (size_t)bytes, file) == (size_t)bytes);
    *count = (uint64_t)bytes / 4;
    exit_code = EXIT_SUCCESS;
cleanup:
    if (file) fclose(file);
    return exit_code;
}

int main(int argc, char **argv) {
    int exit_code = EXIT_FAILURE;
    OgpuError error = {0};
    OgpuProbe *probe = NULL;
    OgpuDevice *device = NULL;
    OgpuKernel *kernels[2] = {0};
    uint32_t *words[2] = {0};
    uint64_t counts[2] = {0};
    REQUIRE(argc == 3);
    for (unsigned i = 0; i < 2; ++i) REQUIRE(read_shader(argv[i + 1], &words[i], &counts[i]) == EXIT_SUCCESS);
    printf("Host-clock measurements: median [min, max], %d warmups + %d samples.\n", WARMUPS, SAMPLES);
    printf("Execution includes recording/submission/wait/cleanup, not isolated GPU time.\n");
    printf("Optional device-batch timings include boundary barriers; query read is separate.\n");
    printf("Validation environment: VK_INSTANCE_LAYERS=%s; VK_LAYER_VALIDATE_SYNC=%s\n",
        getenv("VK_INSTANCE_LAYERS") ? getenv("VK_INSTANCE_LAYERS") : "(unset)",
        getenv("VK_LAYER_VALIDATE_SYNC") ? getenv("VK_LAYER_VALIDATE_SYNC") : "(unset)");
    printf("VK_LOADER_LAYERS_DISABLE=%s (environment reporting is not layer enumeration).\n",
        getenv("VK_LOADER_LAYERS_DISABLE") ? getenv("VK_LOADER_LAYERS_DISABLE") : "(unset)");
    TRY(ogpu_probe_create(OGPU_ABI_VERSION, &probe, &error));
    uint32_t count = 0, tested = 0;
    TRY(ogpu_probe_device_count(probe, &count));
    for (uint32_t i = 0; i < count; ++i) {
        double start = now_ms();
        OgpuResult status = ogpu_device_create(probe, i, &device, &error);
        const double device_ms = now_ms() - start;
        if (status == OGPU_ERROR_UNSUPPORTED) continue;
        TRY(status);
        OgpuDeviceInfo info;
        TRY(ogpu_probe_device_info(probe, i, &info));
        printf("Matrix experiment on %s, Vulkan %" PRIu32 ".%" PRIu32 ".%" PRIu32 "; device creation %.4f ms\n",
            info.name, info.vulkan_api_major, info.vulkan_api_minor, info.vulkan_api_patch, device_ms);
        OgpuTimingInfo timing = {0};
        double wrap_ns = 0;
        OgpuResult timing_status = ogpu_device_timing_info(device, &timing, &error);
        if (timing_status == OGPU_ERROR_UNSUPPORTED) {
            printf("  Device timestamps unsupported: host-only measurements.\n");
        } else {
            TRY(timing_status);
            REQUIRE(timing.reserved == 0 && timing.timestamp_period_ns > 0
                && timing.timestamp_valid_bits >= 36 && timing.timestamp_valid_bits <= 64);
            wrap_ns = timing.timestamp_period_ns;
            for (uint32_t bit = 0; bit < timing.timestamp_valid_bits; ++bit) wrap_ns *= 2;
            printf("  Timestamp clock: %.9g ns/tick, %" PRIu32 " valid bits, wrap %.6g seconds\n",
                timing.timestamp_period_ns, timing.timestamp_valid_bits, wrap_ns / 1e9);
        }
        for (unsigned variant = 0; variant < 2; ++variant) {
            start = now_ms();
            TRY(ogpu_kernel_create(device, words[variant], counts[variant], sizeof(Root), &kernels[variant], &error));
            printf("  %s kernel/pipeline creation %.4f ms\n", names[variant], now_ms() - start);
        }
        const uint32_t shapes[][3] = {{1, 1, 0}, {1, 1, 1}, {3, 5, 7}, {7, 9, 8}, {8, 8, 9},
            {9, 7, 17}, {17, 19, 31}, {31, 17, 33}, {65, 63, 129}, {4, 3, 1025}};
        double max_error[2] = {0}, max_scaled[2] = {0};
        for (unsigned shape = 0; shape < sizeof(shapes) / sizeof(shapes[0]); ++shape)
            for (unsigned pattern = 0; pattern < 5; ++pattern)
                REQUIRE(run_case(device, kernels, shapes[shape][0], shapes[shape][1], shapes[shape][2],
                    pattern, 0, wrap_ns, max_error, max_scaled) == EXIT_SUCCESS);
        printf("Verified 50 shapes/patterns per kernel in %s mode(s), including padding, guards, and unchanged inputs.\n",
            wrap_ns > 0 ? "untimed and timed" : "untimed");
        const uint32_t benchmarks[][3] = {{128, 128, 128}, {257, 193, 129}, {256, 256, 256}};
        for (unsigned shape = 0; shape < sizeof(benchmarks) / sizeof(benchmarks[0]); ++shape)
            REQUIRE(run_case(device, kernels, benchmarks[shape][0], benchmarks[shape][1], benchmarks[shape][2],
                2, 1, wrap_ns, max_error, max_scaled) == EXIT_SUCCESS);
        for (unsigned variant = 0; variant < 2; ++variant) {
            printf("  %s maximum absolute error %.6g; maximum error/bound %.6g\n",
                names[variant], max_error[variant], max_scaled[variant]);
            ogpu_kernel_destroy(kernels[variant]); kernels[variant] = NULL;
        }
        ogpu_device_destroy(device); device = NULL;
        ++tested;
    }
    REQUIRE(tested != 0);
    exit_code = EXIT_SUCCESS;
cleanup:
    for (unsigned i = 0; i < 2; ++i) { ogpu_kernel_destroy(kernels[i]); free(words[i]); }
    ogpu_device_destroy(device);
    ogpu_probe_destroy(probe);
    return exit_code;
}
