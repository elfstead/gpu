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

#include "heap-lower.hpp"
int main(int argc, char **argv) {
    try {
        if (argc == 2 && std::string(argv[1]) == "--test") {
            const std::string vertex_body = "void main() { gl_Position = vec4(position, 0.0, 1.0); }";
            const auto vertex_source = "#version 450\nlayout(location=0) in vec2 uv;\nlayout(location=1) in vec2 position;\n" + vertex_body;
            const std::array<VertexAttribute, 2> attrs{{{0, 0, "uv"}, {1, 8, "position"}}};
            const auto pulled = pull_vertices(vertex_source, attrs, 16, 0);
            const std::string root = "layout(std430, push_constant) uniform PushC {\n"
                "    layout(offset=0) float exposure;\n"
                "    layout(offset=16) mat3 m0;\n"
                "    layout(offset=64) mat3 m1;\n"
                "    layout(offset=112) mat3 m2;\n"
                "    layout(offset=160) mat3 m3;\n};\n";
            auto with_root = vertex_source;
            with_root.insert(with_root.find('\n') + 1, root);
            const auto hdr = pull_vertices(with_root, attrs, 16, 208);
            if (hdr.find("layout(offset=208) ogpu_vertex_data ogpu_vertex_address;") == std::string::npos ||
                hdr.find(root.substr(0, root.size()-3)) == std::string::npos ||
                hdr.substr(hdr.size()-vertex_body.size()) != vertex_body)
                throw std::runtime_error("HDR root offsets/body not preserved");
            for (unsigned bad = 0; bad < 5; ++bad) {
                auto malformed = with_root;
                unsigned bytes = 208;
                if (bad == 0) bytes = 204;
                if (bad == 1) bytes = 212;
                if (bad == 2) malformed.replace(malformed.find("offset=160"), 10, "offset=112");
                if (bad == 3) malformed.replace(malformed.find("mat3 m3"), 7, "vec4 m3");
                if (bad == 4) malformed += root;
                bool rejected = false;
                try { pull_vertices(malformed, attrs, 16, bytes); }
                catch (const std::exception &) { rejected = true; }
                if (!rejected) throw std::runtime_error("malformed HDR root accepted");
            }
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
            for (const auto &format : {"rgba8", "rgba16f"}) {
                unsigned images = 0;
                const auto text = lower(std::string("#version 450\nlayout(binding=2, ") + format +
                    ") writeonly restrict uniform image2D dst;\n" + body, images);
                if (images != 1 || text.find(std::string("layout(descriptor_heap, ") + format + ")") == std::string::npos ||
                    text.substr(text.size()-body.size()) != body)
                    throw std::runtime_error("storage format/body not preserved");
            }
            const auto result = lower("#version 450\nlayout(binding=3) uniform  sampler2D tex;\n" + body, count);
            if (count != 1 || result.substr(result.size() - body.size()) != body ||
                result.find("#define tex sampler2D(ogpu_tex3[3], ogpu_sampler3[3])") == std::string::npos)
                throw std::runtime_error("body preservation / slot mapping test failed");
            for (const auto &bad : {
                "#version 450\nlayout(binding=0) uniform sampler3D unsupported;\n",
                "#version 450\nlayout(binding=2, rgba32f) writeonly restrict uniform image2D unsupported;\n",
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
