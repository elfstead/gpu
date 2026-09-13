#pragma once
#include <regex>
#include <sstream>
#include <stdexcept>
#include <string>

inline std::string lower(const std::string &source, unsigned &resources) {
    const std::regex sampled(R"(^layout\(binding=([0-9]+)\) uniform +sampler([12]D) ([A-Za-z_][A-Za-z_0-9]*);$)");
    const std::regex storage(R"(^layout\(binding=([0-9]+), (rgba8)\) (writeonly restrict )uniform image2D ([A-Za-z_][A-Za-z_0-9]*);$)");
    std::istringstream input(source);
    std::ostringstream output;
    std::string line;
    bool version = false;
    while (std::getline(input, line)) {
        std::smatch match;
        if (line == "#version 450") {
            if (version)
                throw std::runtime_error("duplicate version");
            version = true;
            output << line << "\n#extension GL_EXT_descriptor_heap : require\n";
        } else if (std::regex_match(line, match, sampled)) {
            const auto binding = match[1].str(), dims = match[2].str(), name = match[3].str();
            output << "layout(descriptor_heap) uniform texture" << dims << " ogpu_tex" << binding << "[];\n"
                   << "layout(descriptor_heap) uniform sampler ogpu_sampler" << binding << "[];\n"
                   << "#define " << name << " sampler" << dims << "(ogpu_tex" << binding
                   << "[" << binding << "], ogpu_sampler" << binding << "[" << binding << "])\n";
            ++resources;
        } else if (std::regex_match(line, match, storage)) {
            const auto binding = match[1].str(), name = match[4].str();
            output << "layout(descriptor_heap, rgba8) writeonly restrict uniform image2D ogpu_image"
                   << binding << "[];\n#define " << name << " ogpu_image" << binding
                   << "[" << binding << "]\n";
            ++resources;
        } else {
            // Fail closed on other descriptor declarations; do not quietly emit
            // a mixture of native heaps and descriptor sets.
            if (std::regex_search(line, std::regex(R"(\b(binding|set)\s*=)")))
                throw std::runtime_error("unsupported resource declaration: " + line);
            output << line << '\n';
        }
    }
    if (!version)
        throw std::runtime_error("expected pinned GLSL 450 source");
    return output.str();
}
