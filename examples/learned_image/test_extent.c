#include "extent.h"
#include <assert.h>
#include <stdio.h>

static void selected_extents(void) {
    const uint32_t shapes[][4] = {
        {1280, 720, 2560, 1440}, {1920, 1080, 960, 540},
        {3840, 2160, 4097, 2305}, {3840, 2160, 1919, 1079},
        {1919, 1079, 2561, 1441}, {1919, 1079, 1277, 719},
    };
    const uint32_t maximum[] = {65535, 65535, 65535};
    unsigned partial_rows = 0;
    for (size_t s = 0; s < sizeof(shapes) / sizeof(*shapes); ++s) {
        const uint32_t *shape = shapes[s];
        uint32_t input, output;
        assert(image_count(shape[0], shape[1], 8, &input));
        assert(image_count(shape[2], shape[3], 4, &output));
        assert(resize_axis(shape[0], shape[2]) && resize_axis(shape[1], shape[3]));
        // Compute stages and each guarded diagnostic-poison allocation.
        const uint32_t counts[] = {input * 8, input, output,
            input * 8 + 32, input + 32, output * 4 + 32};
        for (uint32_t threads = 32; threads <= 64; threads *= 2) {
            const uint32_t local[] = {threads, 1, 1};
            for (size_t c = 0; c < sizeof(counts) / sizeof(*counts); ++c) {
                Launch grid;
                assert(launch(counts[c], local, maximum, &grid));
                assert(grid.x == 1024 && grid.y > 1 && grid.stride == 1024 * threads);
                assert((uint64_t)grid.stride * grid.y >= counts[c]);
                assert((uint64_t)grid.stride * (grid.y - 1) < counts[c]);
                assert((uint64_t)grid.stride * grid.y <= UINT32_MAX);
                if (counts[c] % grid.stride) ++partial_rows;
                // The last valid index maps exactly; first tail index is rejected.
                uint32_t last = counts[c] - 1;
                assert(last / grid.stride < grid.y);
                assert(last % grid.stride + (last / grid.stride) * grid.stride == last);
                uint32_t too_short[] = {maximum[0], grid.y - 1, maximum[2]};
                assert(!launch(counts[c], local, too_short, &grid));
            }
        }
    }
    assert(partial_rows > 0);
}

int main(void) {
    selected_extents();
    assert(extent("1920") == 1920 && extent("2147483646") == 2147483646);
    const char *bad[] = {"", "0", "-1", "+1", " 1", "1x", "2147483647", "9999999999999999999999"};
    for (size_t i = 0; i < sizeof(bad) / sizeof(*bad); ++i) assert(!extent(bad[i]));
    uint32_t count;
    assert(image_count(3840, 2160, 8, &count) && count == 8294400);
    assert(!image_count(65536, 65536, 8, &count));
    assert(!image_count(1, 1, 0, &count));
    assert(!image_count(0, 1, 8, &count));
    assert(!image_count(UINT32_MAX, 1, 8, &count));
    assert(!image_count(UINT32_MAX / 8, 1, 8, &count));
    size_t total;
    assert(add_size(3, 4, &total) && total == 7);
    assert(!add_size(SIZE_MAX, 1, &total));
    assert(resize_axis(3840, 7680) && resize_axis(1, 1));
    assert(resize_axis(65535, 32768));
    assert(!resize_axis(65537, 32769));
    assert(!resize_axis(1, INT32_MAX));
    assert(!resize_axis(0, 1) && !resize_axis(1, 0));
    const uint32_t local[] = {64, 1, 1}, limit[] = {3, 4, 1};
    Launch grid;
    assert(launch(193, local, limit, &grid));
    assert(grid.x == 3 && grid.y == 2 && grid.stride == 192);
    // Enumerate a synthetic low-limit launch: each logical index exactly once.
    unsigned seen[193] = {0};
    for (uint32_t y = 0; y < grid.y; ++y)
        for (uint32_t x = 0; x < grid.stride; ++x) {
            uint32_t i = x + y * grid.stride;
            if (i < 193) ++seen[i];
        }
    for (unsigned i = 0; i < 193; ++i) assert(seen[i] == 1);
    assert(launch(768, local, limit, &grid));
    assert(!launch(769, local, limit, &grid));
    assert(!launch(0, local, limit, &grid));
    const uint32_t wide[] = {UINT32_MAX, UINT32_MAX, 1};
    assert(launch(65537, local, wide, &grid));
    assert(grid.x == 1024 && grid.y == 2 && grid.stride == 65536);
    assert(launch(1280u * 720u * 8u, local, wide, &grid));
    assert(grid.x == 1024 && grid.y == 113);
    assert(!launch(UINT32_MAX, local, wide, &grid));
    const uint32_t zero[] = {3, 0, 1}, multidimensional[] = {32, 2, 1};
    assert(!launch(1, local, zero, &grid));
    assert(!launch(1, multidimensional, limit, &grid));
    puts("Checked image extents and X/Y launch boundaries PASS");
    return 0;
}
