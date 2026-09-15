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
    if (stride != 16 || original_push_bytes > 240 || original_push_bytes % 4 != 0 ||
        attributes[0].location != 0 || attributes[0].offset != 0 ||
        attributes[1].location != 1 || attributes[1].offset != 8 ||
        (source.find("push_constant") != std::string::npos) != (original_push_bytes != 0) ||
        source.find("ogpu_vertex_") != std::string::npos)
        throw std::runtime_error("unsupported vertex layout/root");
    const std::regex input(R"(^layout\(location=([01])\) in vec2 ([A-Za-z_][A-Za-z_0-9]*);$)");
    std::istringstream lines(source);
    std::ostringstream result;
    std::string line;
    std::array<bool, 2> seen{};
    bool version = false, in_root = false, root_seen = false;
    uint32_t root_end = 0;
    const uint32_t address_offset = (original_push_bytes + 7u) & ~7u;
    const std::regex member(R"(^    layout\(offset=([0-9]+)\) (float|int|vec2|ivec2|mat3) ([A-Za-z_][A-Za-z_0-9]*);$)");
    while (std::getline(lines, line)) {
        std::smatch match;
        if (line == "#version 450") {
            if (version) throw std::runtime_error("duplicate vertex version");
            version = true;
            result << line << "\n#extension GL_EXT_buffer_reference : require\n"
                "struct ogpu_vertex_record { vec2 a0; vec2 a1; };\n"
                "layout(buffer_reference, std430, buffer_reference_align=8) readonly buffer ogpu_vertex_data { ogpu_vertex_record values[]; };\n";
            if (!original_push_bytes)
                result << "layout(push_constant, std430) uniform ogpu_vertex_root { ogpu_vertex_data vertices; } ogpu_vertex_push;\n";
        } else if (line == "layout(std430, push_constant) uniform PushC {") {
            if (!original_push_bytes || root_seen) throw std::runtime_error("unexpected raster root");
            root_seen = in_root = true;
            result << line << '\n';
        } else if (in_root) {
            if (line == "};") {
                if (root_end != original_push_bytes) throw std::runtime_error("raster root size mismatch");
                result << "    layout(offset=" << address_offset << ") ogpu_vertex_data ogpu_vertex_address;\n};\n";
                in_root = false;
            } else {
                if (!std::regex_match(line, match, member)) throw std::runtime_error("unsupported root member");
                const auto offset = std::stoul(match[1]);
                const auto type = match[2].str();
                const unsigned size = type == "mat3" ? 48 : type == "vec2" || type == "ivec2" ? 8 : 4;
                const unsigned align = type == "mat3" ? 16 : size;
                if (offset % align || offset < root_end || offset > original_push_bytes ||
                    size > original_push_bytes - offset)
                    throw std::runtime_error("invalid root member offset");
                root_end = offset + size;
                result << line << '\n';
            }
        } else if (std::regex_match(line, match, input)) {
            const auto index = static_cast<unsigned>(std::stoul(match[1]));
            if (seen[index] || attributes[index].name != match[2])
                throw std::runtime_error("vertex declaration/metadata mismatch");
            seen[index] = true;
            result << "#define " << match[2] << (original_push_bytes ? " ogpu_vertex_address" : " ogpu_vertex_push.vertices")
                   << ".values[gl_VertexIndex].a" << index << '\n';
        } else {
            if (line.find("push_constant") != std::string::npos ||
                std::regex_search(line, std::regex(R"(\bin\s+\w+\s+\w+\s*[;\[])")))
                throw std::runtime_error("unsupported vertex input: " + line);
            result << line << '\n';
        }
    }
    if (!version || !seen[0] || !seen[1] || in_root || (original_push_bytes && !root_seen))
        throw std::runtime_error("missing pinned vertex inputs");
    // getline adds a final newline; retain the original final statement bytes.
    auto text = result.str();
    if (!source.empty() && source.back() != '\n') text.pop_back();
    return text;
}
