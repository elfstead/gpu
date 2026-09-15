// Compare separately executed backends, including their exact input bytes.
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define CHECK(x) do { if (!(x)) { fprintf(stderr, "HDR comparison failed at line %d: %s\n", __LINE__, #x); exit(1); } } while (0)
static void *read_image(const char *dir, int w, int h, int frame, const char *suffix, size_t bytes)
{
    char path[4096];
    int n = snprintf(path, sizeof(path), "%s/%dx%d-frame%d%s", dir, w, h, frame, suffix);
    CHECK(n > 0 && n < (int) sizeof(path));
    FILE *f = fopen(path, "rb");
    if (!f) fprintf(stderr, "Cannot read %s\n", path);
    CHECK(f);
    void *data = malloc(bytes); CHECK(data);
    CHECK(fread(data, 1, bytes, f) == bytes && fgetc(f) == EOF && !ferror(f));
    CHECK(!fclose(f));
    return data;
}
static float wide(uint16_t bits)
{
    _Static_assert(sizeof(_Float16) == 2, "binary16 comparison");
    _Float16 value; memcpy(&value, &bits, 2); return value;
}
static int within(float a, float b)
{ return isfinite(a) && isfinite(b) && fabsf(a-b) <= 0.005f + 0.005f * fabsf(a); }
int main(int argc, char **argv)
{
    CHECK(argc == 3);
    CHECK(within(4, 4.02f) && !within(4, 4.03f) && !within(NAN, 0) && !within(1, INFINITY));
    const int sizes[][2] = {{16,16}, {31,17}, {64,33}};
    for (unsigned s = 0; s < 3; ++s) for (int frame = 0; frame < 3; ++frame) {
        int w = sizes[s][0], h = sizes[s][1];
        size_t in = (size_t) w*h*8, n = (size_t) (2*w+1)*(2*h+1)*4;
        void *ia = read_image(argv[1],w,h,frame,"-input.rgba16f",in);
        void *ib = read_image(argv[2],w,h,frame,"-input.rgba16f",in);
        CHECK(!memcmp(ia,ib,in)); free(ia); free(ib);
        uint16_t *a = read_image(argv[1],w,h,frame,".rgba16f",n*2);
        uint16_t *b = read_image(argv[2],w,h,frame,".rgba16f",n*2);
        uint8_t *ra = read_image(argv[1],w,h,frame,".rgba",n);
        uint8_t *rb = read_image(argv[2],w,h,frame,".rgba",n);
        float max_float = 0; unsigned max_rgb = 0;
        for (size_t i = 0; i < n; ++i) {
            float av = wide(a[i]), bv = wide(b[i]);
            CHECK(within(av,bv)); max_float = fmaxf(max_float,fabsf(av-bv));
            if (i%4 == 3) {
                CHECK(fabsf(av-1) <= 0x1p-10f && fabsf(bv-1) <= 0x1p-10f);
                CHECK(ra[i] == 255 && rb[i] == 255);
            } else {
                unsigned diff = abs((int)ra[i]-(int)rb[i]);
                if (diff > 2) fprintf(stderr,"%dx%d frame=%d component=%zu native=%u ogpu=%u diff=%u\n",w,h,frame,i,ra[i],rb[i],diff);
                CHECK(diff <= 2); if (diff > max_rgb) max_rgb = diff;
            }
        }
        printf("HDR-compare=%dx%d frame=%d max_intermediate=%g max_rgb=%u PASS\n",w,h,frame,max_float,max_rgb);
        free(a); free(b); free(ra); free(rb);
    }
    puts("HDR-comparison cases=9 PASS");
}
