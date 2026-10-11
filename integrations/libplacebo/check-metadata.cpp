// Host-only tests; original artifacts remain specialized by the native driver.
#include "compiler.h"
#include "shader-metadata.hpp"
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <memory>

#define CHECK(x) do { if (!(x)) throw std::runtime_error(#x); } while (0)
static void check_source(const char *source, const std::vector<shader_constant> &values,
                         uint32_t x, uint32_t y, uint32_t shared, bool rejected = false) {
    shader_binary binary{};
    CHECK(compile_native(source, 0, nullptr, nullptr, 0, &binary));
    std::unique_ptr<uint32_t, decltype(&std::free)> owner(binary.words, std::free);
    const std::vector<uint32_t> original(binary.words, binary.words + binary.count);
    bool failed = false;
    shader_metadata result;
    try { result = compute_metadata(binary.words, binary.count, values); }
    catch (const std::exception &) { failed = true; }
    CHECK(failed == rejected);
    CHECK(!std::memcmp(original.data(), binary.words, binary.count * 4));
    if (!rejected) CHECK(result.local[0] == x && result.local[1] == y && result.local[2] == 1 && result.shared == shared);
}
int main() {
    try {
        const char *source = "#version 450\n"
            "layout(local_size_x=8, local_size_y=4) in;\n"
            "layout(constant_id=3) const int width=2;\n"
            "layout(constant_id=7) const int height=3;\n"
            "shared vec2 base; shared float plane[width*height];\n"
            "void main() { base=vec2(1); plane[gl_LocalInvocationIndex % (width*height)]=base.x; }\n";
        check_source(source, {}, 8, 4, 32);
        check_source(source, {{3,5},{7,7}}, 8, 4, 148);
        check_source(source, {{3,2},{7,3}}, 8, 4, 32); // A/B/A, no cached stale metadata
        check_source(source, {{3,2},{3,3}}, 0, 0, 0, true);
        check_source(source, {{3,UINT32_MAX}}, 0, 0, 0, true);
        check_source("#version 450\nlayout(local_size_x_id=2) in;\nvoid main() {}\n", {{2,16}}, 16, 1, 0);
        check_source("#version 450\nlayout(local_size_x=1) in;\nshared vec4 a[2][3];\nvoid main(){ a[0][0]=vec4(1); }\n", {}, 1, 1, 96);
        check_source("#version 450\nlayout(local_size_x=1) in;\nshared vec3 a;\nvoid main(){ a=vec3(1); }\n", {}, 0, 0, 0, true);
        check_source("#version 450\nlayout(local_size_x=1) in;\nstruct S { float x; vec2 y; };\nshared S a;\nvoid main(){ a.x=1; }\n", {}, 0, 0, 0, true);
        for (const std::vector<uint32_t> &malformed : {std::vector<uint32_t>{}, {0x07230203,0x10600,0,1,0,0}}) {
            bool failed = false;
            try { (void)compute_metadata(malformed.data(), malformed.size(), {}); }
            catch (const std::exception &) { failed = true; }
            CHECK(failed);
        }
        std::cout << "Compiler metadata specialization, shared arrays, rejection and artifact preservation PASS\n";
    } catch (const std::exception &error) { std::cerr << error.what() << '\n'; return 1; }
}
