// Bounded lowering for the pinned consumer's two vec2 attributes and 16-byte
// vertex stride. Processing statements are preserved; metadata must match exactly.
#pragma once
#include <array>
#include <cstdint>
#include <regex>
#include <sstream>
#include <stdexcept>
#include <string>

struct VertexAttribute {
    uint32_t location, offset;
    std::string name;
};

inline std::string pull_vertices(const std::string &source,
                                const std::array<VertexAttribute, 2> &attributes,
                                uint32_t stride, uint32_t original_push_bytes) {
    if (stride != 16 || original_push_bytes != 0 ||
        attributes[0].location != 0 || attributes[0].offset != 0 ||
        attributes[1].location != 1 || attributes[1].offset != 8 ||
        source.find("push_constant") != std::string::npos ||
        source.find("ogpu_vertex_") != std::string::npos)
        throw std::runtime_error("unsupported vertex layout/root");
    const std::regex input(R"(^layout\(location=([01])\) in vec2 ([A-Za-z_][A-Za-z_0-9]*);$)");
    std::istringstream lines(source);
    std::ostringstream result;
    std::string line;
    std::array<bool, 2> seen{};
    bool version = false;
    while (std::getline(lines, line)) {
        std::smatch match;
        if (line == "#version 450") {
            if (version) throw std::runtime_error("duplicate vertex version");
            version = true;
            result << line << "\n#extension GL_EXT_buffer_reference : require\n"
                "struct ogpu_vertex_record { vec2 a0; vec2 a1; };\n"
                "layout(buffer_reference, std430, buffer_reference_align=8) readonly buffer ogpu_vertex_data { ogpu_vertex_record values[]; };\n"
                "layout(push_constant, std430) uniform ogpu_vertex_root { ogpu_vertex_data vertices; } ogpu_vertex_push;\n";
        } else if (std::regex_match(line, match, input)) {
            const auto index = static_cast<unsigned>(std::stoul(match[1]));
            if (seen[index] || attributes[index].name != match[2])
                throw std::runtime_error("vertex declaration/metadata mismatch");
            seen[index] = true;
            result << "#define " << match[2] << " ogpu_vertex_push.vertices.values[gl_VertexIndex].a" << index << '\n';
        } else {
            if (std::regex_search(line, std::regex(R"(\bin\s+\w+\s+\w+\s*[;\[])")))
                throw std::runtime_error("unsupported vertex input: " + line);
            result << line << '\n';
        }
    }
    if (!version || !seen[0] || !seen[1])
        throw std::runtime_error("missing pinned vertex inputs");
    // getline adds a final newline; retain the original final statement bytes.
    auto text = result.str();
    if (!source.empty() && source.back() != '\n') text.pop_back();
    return text;
}
