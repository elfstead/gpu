// Shared acceptance workload: identical upstream generation/dispatch and input
// for independent reference and OGPU executables. No implementation fallback.
#pragma once
#include <libplacebo/shaders/sampling.h>
#include <libplacebo/dispatch.h>
#ifndef OGPU_STREAM_WORKLOAD
struct workload_counts { unsigned creates, compute, raster; };
static struct workload_counts workload_counts(pl_gpu gpu);
#endif

static void pattern(uint8_t *data, int w, int h, unsigned frame)
{
    for (int y = 0; y < h; ++y) {
        for (int x = 0; x < w; ++x) {
            size_t i = ((size_t) y * w + x) * 4;
            data[i] = (x * 7 + y * 3 + frame * 19) % 256;
            data[i + 1] = (x * 5 + y * 11 + frame * 23) % 256;
            data[i + 2] = ((x ^ y) * 13 + frame * 29) % 256;
            data[i + 3] = 255;
        }
    }
}

static uint64_t checksum(const uint8_t *data, size_t size)
{
    uint64_t hash = UINT64_C(14695981039346656037);
    for (size_t i = 0; i < size; ++i)
        hash = (hash ^ data[i]) * UINT64_C(1099511628211);
    return hash;
}

static void process_frame(pl_gpu gpu, pl_dispatch dp, pl_shader_obj *lut,
                          pl_tex src, pl_tex mid, pl_tex dst, int ow, int oh)
{
    pl_dispatch_reset_frame(dp);
    pl_shader sh = pl_dispatch_begin(dp);
    CHECK(pl_shader_sample_polar(sh, pl_sample_src(.tex = src, .new_w = ow, .new_h = oh),
        pl_sample_filter_params(.filter = pl_filter_ewa_lanczos, .lut = lut)));
    CHECK(pl_shader_is_compute(sh));
    CHECK(pl_dispatch_finish(dp, pl_dispatch_params(.shader = &sh, .target = mid)));
    sh = pl_dispatch_begin(dp);
    CHECK(pl_shader_sample_nearest(sh, pl_sample_src(.tex = mid)));
    CHECK(!pl_shader_is_compute(sh));
    CHECK(pl_dispatch_finish(dp, pl_dispatch_params(.shader = &sh, .target = dst)));
    CHECK(!pl_gpu_is_failed(gpu));
}

#ifndef OGPU_STREAM_WORKLOAD
static void run_case(pl_gpu gpu, int w, int h, const char *label)
{
    // Upscale by an odd, non-integer ratio, exercising workgroup tails.
    const int ow = w * 2 + 1, oh = h * 2 + 1;
    const size_t in_size = (size_t) w * h * 4, out_size = (size_t) ow * oh * 4;
    uint8_t *input = malloc(in_size), *middle = malloc(out_size), *output = malloc(out_size);
    uint8_t *first = malloc(out_size);
    CHECK(input && middle && output && first);
    pl_fmt rgba = pl_find_named_fmt(gpu, "rgba8");
    CHECK(rgba && (rgba->caps & PL_FMT_CAP_STORABLE));
    pl_tex src = pl_tex_create(gpu, pl_tex_params(
        .w = w, .h = h, .format = rgba, .sampleable = true, .host_writable = true));
    pl_tex mid = pl_tex_create(gpu, pl_tex_params(
        .w = ow, .h = oh, .format = rgba, .sampleable = true,
        .storable = true, .renderable = true, .host_readable = true));
    // The last pass has a genuine raster target, not a compute copy of its output.
    pl_tex dst = pl_tex_create(gpu, pl_tex_params(
        .w = ow, .h = oh, .format = rgba, .renderable = true, .host_readable = true));
    CHECK(src && mid && dst);
    pl_dispatch dp = pl_dispatch_create(gpu->log, gpu);
    CHECK(dp);
    pl_shader_obj lut = NULL;
    const struct workload_counts initial = workload_counts(gpu);
    unsigned warmed_creates = 0;
    for (unsigned frame = 0; frame < 3; ++frame) {
        // A/B/A checks both live updates and return to a deterministic earlier result.
        pattern(input, w, h, frame == 1);
        CHECK(pl_tex_upload(gpu, pl_tex_transfer_params(.tex = src, .ptr = input)));
        process_frame(gpu, dp, &lut, src, mid, dst, ow, oh);
        const struct workload_counts current = workload_counts(gpu);
        CHECK(current.compute - initial.compute == frame + 1);
        CHECK(current.raster - initial.raster == frame + 1);
        CHECK(pl_tex_download(gpu, pl_tex_transfer_params(.tex = mid, .ptr = middle)));
        CHECK(pl_tex_download(gpu, pl_tex_transfer_params(.tex = dst, .ptr = output)));
        CHECK(memcmp(middle, output, out_size) == 0);
        for (size_t i = 3; i < out_size; i += 4)
            CHECK(output[i] == 255);
        if (frame == 0) {
            memcpy(first, output, out_size);
            warmed_creates = current.creates;
        } else {
            CHECK(current.creates == warmed_creates); // upstream executable reuse
            CHECK((memcmp(first, output, out_size) == 0) == (frame == 2));
        }
        printf("%s=%dx%d output=%dx%d frame=%u checksum=%016" PRIx64 " compute=1 raster=1 PASS\n",
               label, w, h, ow, oh, frame, checksum(output, out_size));
        // Raw outputs are local artifacts; generated upstream code is not vendored.
        char suffix[128];
        snprintf(suffix, sizeof(suffix), "%dx%d-frame%u.rgba", w, h, frame);
        save(current.creates, suffix, output, out_size);
        snprintf(suffix, sizeof(suffix), "%dx%d-frame%u-middle.rgba", w, h, frame);
        save(current.creates, suffix, middle, out_size);
    }
    pl_dispatch_destroy(&dp);
    pl_shader_obj_destroy(&lut);
    pl_tex_destroy(gpu, &dst);
    pl_tex_destroy(gpu, &mid);
    pl_tex_destroy(gpu, &src);
    free(first); free(output); free(middle); free(input);
}
#endif
