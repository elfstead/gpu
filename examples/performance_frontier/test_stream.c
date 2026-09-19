#define FRONTIER_STREAM_NO_MAIN
#include "stream.c"
#include <assert.h>
int main(void) {
    (void)create; (void)window; (void)selftest;
    (void)stream_create; (void)stream_destroy; (void)stream_window; (void)stream_check_buffers;
    unsigned char pixels[2 * SG + 8];
    unsigned char reference[] = {10, 20, 30, 255, 90, 100, 110, 255};
    Stream s = {.final_size=sizeof(pixels), .pixels=pixels, .reference={reference, reference}};
    memset(pixels, 0xa5, sizeof(pixels)); memcpy(pixels + SG, reference, sizeof(reference));
    assert(stream_verify(&s, 0) && s.maximum_delta == 0);
    pixels[SG] += 1; assert(stream_verify(&s, 0) && s.maximum_delta == 1);
    pixels[SG] += 1; assert(!stream_verify(&s, 0)); pixels[SG] -= 2;
    pixels[SG + 3] = 254; assert(!stream_verify(&s, 0)); pixels[SG + 3] = 255;
    pixels[0] = 0; assert(!stream_verify(&s, 0)); pixels[0] = 0xa5;
    pixels[sizeof(pixels) - 1] = 0; assert(!stream_verify(&s, 0));
    puts("Streaming exact-alpha/RGB-bound/prefix/suffix host checks PASS");
    return 0;
}
