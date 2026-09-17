#include "extent.h"
#include <assert.h>
#include <stdio.h>

int main(void) {
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
    assert(!launch(UINT32_MAX, local, wide, &grid));
    const uint32_t zero[] = {3, 0, 1}, multidimensional[] = {32, 2, 1};
    assert(!launch(1, local, zero, &grid));
    assert(!launch(1, multidimensional, limit, &grid));
    puts("Checked image extents and X/Y launch boundaries PASS");
    return 0;
}
