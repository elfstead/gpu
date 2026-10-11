#include "shader-metadata.hpp"
#include <iostream>
extern "C" int reflect_compute_native(const shader_binary *code, const shader_constant *values,
                                       uint32_t count, shader_metadata *out) {
    *out = {};
    try {
        std::vector<shader_constant> constants;
        if (count) constants.assign(values, values + count);
        *out = compute_metadata(code->words, code->count, constants);
        return 1;
    } catch (const std::exception &error) {
        std::cerr << "native metadata: " << error.what() << '\n';
        return 0;
    }
}
