// Bounded compiler probe for pinned libplacebo-generated declarations, NOT a GLSL
// parser or backend. Preserve the processing body and specialization constants.
// Native resource/sampler slots correspond to the original binding numbers.
#include <fstream>
#include <iostream>
#include <regex>
#include <sstream>
#include <stdexcept>
#include <string>
#include "vertex-pull.hpp"

static std::string lower(const std::string &source, unsigned &resources) {
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

int main(int argc, char **argv) {
    try {
        if (argc == 2 && std::string(argv[1]) == "--test") {
            const std::string vertex_body = "void main() { gl_Position = vec4(position, 0.0, 1.0); }";
            const auto vertex_source = "#version 450\nlayout(location=0) in vec2 uv;\nlayout(location=1) in vec2 position;\n" + vertex_body;
            const std::array<VertexAttribute, 2> attrs{{{0, 0, "uv"}, {1, 8, "position"}}};
            const auto pulled = pull_vertices(vertex_source, attrs, 16, 0);
            if (pulled.substr(pulled.size() - vertex_body.size()) != vertex_body ||
                pulled.find("ogpu_vertex_push.vertices.values[gl_VertexIndex].a1") == std::string::npos)
                throw std::runtime_error("vertex body preservation / address fetching failed");
            for (unsigned bad = 0; bad < 7; ++bad) {
                bool rejected = false;
                auto wrong = attrs;
                if (bad == 0) wrong[0].offset = 8;
                if (bad == 1) wrong[1].name = "wrong";
                if (bad == 2) wrong[1].location = 2;
                auto input = vertex_source;
                if (bad == 5) input += "\nlayout(location=2) in vec3 extra;\n";
                if (bad == 6) input += "\nlayout(push_constant) uniform Existing { float v; };\n";
                try { pull_vertices(input, wrong, bad == 3 ? 32 : 16, bad == 4 ? 8 : 0); }
                catch (const std::exception &) { rejected = true; }
                if (!rejected) throw std::runtime_error("unsupported vertex layout accepted");
            }
            const std::string body = "void main() { vec4 c = textureLod(tex, vec2(0.5), 0.0); }\n";
            unsigned count = 0;
            const auto result = lower("#version 450\nlayout(binding=3) uniform  sampler2D tex;\n" + body, count);
            if (count != 1 || result.substr(result.size() - body.size()) != body ||
                result.find("#define tex sampler2D(ogpu_tex3[3], ogpu_sampler3[3])") == std::string::npos)
                throw std::runtime_error("body preservation / slot mapping test failed");
            for (const auto &bad : {
                "#version 450\nlayout(binding=0) uniform sampler3D unsupported;\n",
                "#version 450\nlayout(set = 0, binding = 1) uniform sampler2D unsupported;\n",
                "#version 460\nvoid main() {}\n"}) {
                bool rejected = false;
                try { unsigned ignored = 0; lower(bad, ignored); }
                catch (const std::exception &) { rejected = true; }
                if (!rejected)
                    throw std::runtime_error("unsupported input not rejected");
            }
            std::cout << "heap-lower resource/vertex body preservation, slots, metadata and rejection tests PASS\n";
            return 0;
        }
        if (argc != 3 && argc != 6)
            throw std::runtime_error("usage: heap-lower input.glsl output.glsl [--vertex location0-name location1-name]");
        std::ifstream input(argv[1]);
        if (!input)
            throw std::runtime_error("cannot open input");
        std::ostringstream source;
        source << input.rdbuf();
        if (input.bad())
            throw std::runtime_error("input read failed");
        unsigned count = 0;
        auto text = source.str();
        if (argc == 6) {
            if (std::string(argv[3]) != "--vertex") throw std::runtime_error("expected --vertex");
            text = pull_vertices(text, {{{0, 0, argv[4]}, {1, 8, argv[5]}}}, 16, 0);
        }
        const auto result = lower(text, count);
        std::ofstream output(argv[2]);
        output << result;
        output.close();
        if (!output)
            throw std::runtime_error("output write failed");
        std::cout << "heap-lower resources=" << count << " source=" << argv[1] << '\n';
    } catch (const std::exception &e) {
        std::cerr << e.what() << '\n';
        return 1;
    }
}
