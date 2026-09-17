// Scalar binary64 oracle, independent of GPU code. Compile with contraction off.
// Temporary storage is O(input width + output width), never O(image area).
#define _POSIX_C_SOURCE 200809L
#define _FILE_OFFSET_BITS 64
#include "extent.h"
#include <math.h>
#include <stdio.h>
#include <string.h>

static void check(int okay, const char *message) {
    if (!okay) { fprintf(stderr, "Streaming reference: %s\n", message); exit(EXIT_FAILURE); }
}
static FILE *open_file(const char *directory, const char *name, const char *mode) {
    char path[4096];
    int n = snprintf(path, sizeof(path), "%s/%s", directory, name);
    check(n > 0 && (size_t)n < sizeof(path), "path too long");
    FILE *f = fopen(path, mode);
    check(f != NULL, path);
    return f;
}
static void close_file(FILE *f) { check(fclose(f) == 0, "close failed"); }
static void *allocate(size_t count, size_t size) {
    check(count <= SIZE_MAX / size, "allocation overflow");
    void *p = calloc(count, size);
    check(p != NULL, "allocation failed");
    return p;
}
static void write_row(FILE *f, const void *p, size_t size, size_t count) {
    check(fwrite(p, size, count, f) == count, "write failed");
}
static void read_row(FILE *f, void *p, size_t size, uint32_t width, uint32_t y) {
    uint64_t offset = (uint64_t)y * width * size;
    check(offset <= INT64_MAX && fseeko(f, (off_t)offset, SEEK_SET) == 0, "row seek failed");
    check(fread(p, size, width, f) == width, "truncated row");
}
static double clamp(double v) { return fmin(1.0, fmax(0.0, v)); }
static uint32_t bounded(int64_t v, uint32_t size) {
    return v < 0 ? 0 : (uint64_t)v >= size ? size - 1 : (uint32_t)v;
}
static uint32_t word(uint32_t *state) {
    uint32_t x = *state;
    x ^= x << 13; x ^= x >> 17; x ^= x << 5;
    return *state = x;
}
static double uniform(uint32_t *state) { return (word(state) >> 8) / 16777216.0; }

static void scene(const char *directory, uint32_t w, uint32_t h, uint32_t seed) {
    double base = 0.15 + 0.7 * uniform(&seed);
    double dx = (uniform(&seed) - 0.5) * 0.5;
    double dy = (uniform(&seed) - 0.5) * 0.5;
    struct Rectangle { uint32_t x0, x1, y0, y1; double level; } boxes[5];
    for (unsigned i = 0; i < 5; ++i) {
        uint32_t a = word(&seed) % w, b = word(&seed) % w;
        boxes[i].x0 = a < b ? a : b; boxes[i].x1 = a > b ? a : b;
        a = word(&seed) % h; b = word(&seed) % h;
        boxes[i].y0 = a < b ? a : b; boxes[i].y1 = a > b ? a : b;
        boxes[i].level = 0.05 + 0.9 * uniform(&seed);
    }
    float *clean = allocate(w, sizeof(float)), *input = allocate(w, sizeof(float));
    FILE *cf = open_file(directory, "clean.f32", "wb"), *nf = open_file(directory, "input.f32", "wb");
    for (uint32_t y = 0; y < h; ++y) {
        for (uint32_t x = 0; x < w; ++x) {
            double value = clamp(base + dx * ((double)x / (w > 1 ? w - 1 : 1) - 0.5)
                + dy * ((double)y / (h > 1 ? h - 1 : 1) - 0.5));
            for (unsigned i = 0; i < 5; ++i)
                if (x >= boxes[i].x0 && x <= boxes[i].x1 && y >= boxes[i].y0 && y <= boxes[i].y1)
                    value = boxes[i].level;
            clean[x] = (float)value;
            double noise = 0;
            for (unsigned i = 0; i < 12; ++i) noise += uniform(&seed) - 0.5;
            input[x] = (float)((double)clean[x] + 0.1 * noise);
        }
        write_row(cf, clean, sizeof(float), w); write_row(nf, input, sizeof(float), w);
    }
    close_file(cf); close_file(nf); free(clean); free(input);
}

static void infer(const char *directory, uint32_t w, uint32_t h, const float weights[89]) {
    FILE *input = open_file(directory, "input.f32", "rb");
    FILE *hf = open_file(directory, "hidden.f64", "wb"), *df = open_file(directory, "denoised.f64", "wb");
    float *rows = allocate((size_t)w * 3, sizeof(float));
    double *hidden = allocate((size_t)w * 8, sizeof(double)), *denoised = allocate(w, sizeof(double));
    for (uint32_t y = 0; y < h; ++y) {
        for (int ky = -1; ky <= 1; ++ky)
            read_row(input, rows + (size_t)(ky + 1) * w, sizeof(float), w, bounded((int64_t)y + ky, h));
        for (uint32_t x = 0; x < w; ++x) {
            double patch[9];
            for (int ky = -1; ky <= 1; ++ky) for (int kx = -1; kx <= 1; ++kx)
                patch[(ky + 1) * 3 + kx + 1] = rows[(size_t)(ky + 1) * w + bounded((int64_t)x + kx, w)];
            double correction = weights[88];
            for (unsigned c = 0; c < 8; ++c) {
                double value = weights[72 + c];
                for (unsigned k = 0; k < 9; ++k) value += (patch[k] - 0.5) * weights[c * 9 + k];
                value = fmax(0.0, value);
                hidden[(size_t)x * 8 + c] = value;
                correction += value * weights[80 + c];
            }
            denoised[x] = clamp((double)rows[w + x] + correction);
        }
        write_row(hf, hidden, sizeof(double), (size_t)w * 8);
        write_row(df, denoised, sizeof(double), w);
    }
    close_file(input); close_file(hf); close_file(df);
    free(rows); free(hidden); free(denoised);
}

static void render(const char *directory, uint32_t w, uint32_t h, uint32_t ow, uint32_t oh) {
    FILE *df = open_file(directory, "denoised.f64", "rb");
    FILE *pf = open_file(directory, "processed.f64", "wb"), *ff = open_file(directory, "final.rgba", "wb");
    double *top = allocate(w, sizeof(double)), *bottom = allocate(w, sizeof(double));
    double *color = allocate((size_t)ow * 4, sizeof(double));
    unsigned char *pixels = allocate((size_t)ow * 4, 1);
    for (uint32_t y = 0; y < oh; ++y) {
        double sy = (y + 0.5) * h / oh - 0.5;
        int64_t iy = (int64_t)floor(sy);
        double fy = sy - iy;
        read_row(df, top, sizeof(double), w, bounded(iy, h));
        read_row(df, bottom, sizeof(double), w, bounded(iy + 1, h));
        for (uint32_t x = 0; x < ow; ++x) {
            double sx = (x + 0.5) * w / ow - 0.5;
            int64_t ix = (int64_t)floor(sx);
            double fx = sx - ix;
            uint32_t a = bounded(ix, w), b = bounded(ix + 1, w);
            double t = top[a] * (1 - fx) + top[b] * fx;
            double u = bottom[a] * (1 - fx) + bottom[b] * fx;
            double v = t * (1 - fy) + u * fy;
            double *p = color + (size_t)x * 4;
            p[0] = 0.95 * v; p[1] = 0.8 * v + 0.05; p[2] = 0.6 * v + 0.15; p[3] = 1.0;
            for (unsigned c = 0; c < 4; ++c) pixels[(size_t)x * 4 + c] = (unsigned char)floor(clamp(p[c]) * 255 + 0.5);
        }
        write_row(pf, color, sizeof(double), (size_t)ow * 4);
        write_row(ff, pixels, 1, (size_t)ow * 4);
    }
    close_file(df); close_file(pf); close_file(ff);
    free(top); free(bottom); free(color); free(pixels);
}

int main(int argc, char **argv) {
    check(argc == 8, "usage: reference-stream weights directory width height out_width out_height seed");
    const uint16_t endian = 1;
    check(*(const unsigned char *)&endian == 1 && sizeof(float) == 4 && sizeof(double) == 8,
          "requires little-endian binary32/binary64 host");
    uint32_t w = extent(argv[3]), h = extent(argv[4]), ow = extent(argv[5]), oh = extent(argv[6]);
    uint32_t seed = extent(argv[7]), count;
    check(seed && image_count(w, h, 8, &count) && image_count(ow, oh, 4, &count), "invalid extents/seed");
    float weights[89];
    FILE *wf = fopen(argv[1], "rb");
    check(wf != NULL, "open weights failed");
    check(fread(weights, sizeof(float), 89, wf) == 89 && fgetc(wf) == EOF && !ferror(wf), "invalid weights length");
    close_file(wf);
    for (unsigned i = 0; i < 89; ++i) check(isfinite(weights[i]), "nonfinite weight");
    scene(argv[2], w, h, seed);
    infer(argv[2], w, h, weights);
    render(argv[2], w, h, ow, oh);
    // Excludes stdio buffers, stack and allocator overhead; independent of height.
    size_t inference_bytes = (size_t)w * (3 * sizeof(float) + 9 * sizeof(double));
    size_t render_bytes = (size_t)w * 2 * sizeof(double) + (size_t)ow * (4 * sizeof(double) + 4);
    printf("Streaming FP64 oracle %ux%u -> %ux%u; peak row payload=%zu bytes\n", w, h, ow, oh,
        inference_bytes > render_bytes ? inference_bytes : render_bytes);
    return 0;
}
