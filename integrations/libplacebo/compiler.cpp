#include "compiler.h"
#include "heap-lower.hpp"
#include "vertex-pull.hpp"
#include <shaderc/shaderc.hpp>
#include <cstdlib>
#include <cstring>
#include <iostream>

extern "C" int compile_native(const char *source, int stage, const char *uv,
                              const char *pos, shader_binary *out) {
    *out = {};
    try {
        std::string text = source;
        if (stage == 2)
            text = pull_vertices(text, {{{0, 0, uv}, {1, 8, pos}}}, 16, 0);
        unsigned resources = 0;
        text = lower(text, resources);
        shaderc::Compiler compiler;
        shaderc::CompileOptions options;
        options.SetTargetEnvironment(shaderc_target_env_vulkan, shaderc_env_version_vulkan_1_4);
        const auto kind = stage == 0 ? shaderc_compute_shader :
                          stage == 1 ? shaderc_fragment_shader : shaderc_vertex_shader;
        const auto result = compiler.CompileGlslToSpv(text, kind, "libplacebo-native", options);
        if (result.GetCompilationStatus() != shaderc_compilation_status_success)
            throw std::runtime_error(result.GetErrorMessage());
        out->count = result.cend() - result.cbegin();
        out->words = static_cast<uint32_t *>(std::malloc(out->count * 4));
        if (!out->words) throw std::bad_alloc();
        std::memcpy(out->words, result.cbegin(), out->count * 4);
        return 1;
    } catch (const std::exception &error) {
        std::cerr << "native compiler: " << error.what() << '\n';
        return 0;
    }
}
