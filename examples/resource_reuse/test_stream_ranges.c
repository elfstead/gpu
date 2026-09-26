#define FRONTIER_REUSE
#define FRONTIER_STREAM_NO_MAIN
#include "../performance_frontier/stream.c"

static int test_ranges(void) {
    (void)create; (void)window; (void)selftest;
    (void)stream_create; (void)stream_destroy; (void)stream_window; (void)stream_check_buffers;
    (void)stream_check_range_padding;
    Context c = {0}; unsigned char data[256], output[8], input[8] = {1,2,3,4,5,6,7,8};
    memset(data, 0xa5, sizeof(data));
    SBuffer parent = {.size=sizeof(data), .view={.data=data, .size_bytes=sizeof(data),
        .alignment=1, .access_granularity=1, .coherent=1}};
    SBuffer first = parent, second = parent;
    first.offset = 64; first.size = 8;
    second.offset = 192; second.size = 8;
    CHECK(sb_write(&c, &first, input, sizeof(input)));
    CHECK(sb_read(&c, &first, output, sizeof(output)) && !memcmp(input, output, sizeof(input)));
    CHECK(sb_write(&c, &second, input, sizeof(input)));
    SBuffer *ranges[] = {&first, &second};
    CHECK(stream_padding(&c, &parent, ranges, 2));
    data[0] = 0; CHECK(!stream_padding(&c, &parent, ranges, 2)); data[0] = 0xa5;
    data[100] = 0; CHECK(!stream_padding(&c, &parent, ranges, 2)); data[100] = 0xa5;
    data[255] = 0; CHECK(!stream_padding(&c, &parent, ranges, 2)); data[255] = 0xa5;
    second.offset = 65; CHECK(!stream_padding(&c, &parent, ranges, 2));
    second.offset = UINT64_MAX; CHECK(!stream_padding(&c, &parent, ranges, 2));
    second.offset = 250; CHECK(!stream_padding(&c, &parent, ranges, 2));
    CHECK(!sb_write(&c, &first, input, 9) && !sb_read(&c, &first, output, 9));
    puts("Mapped slice bounds/prefix/gap/suffix negative tests PASS (coherent CPU fixture)");
    return 1;
}
int main(void) { return !test_ranges(); }
