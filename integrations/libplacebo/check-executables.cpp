// G1 only: create prepared OGPU executables from captured upstream shaders and
// their exact constant bits. No dispatch/draw and no image-comparison claim.
#include "ogpu_next.h"
#include "shader-metadata.hpp"
#include <filesystem>
#include <fstream>
#include <iostream>
#include <memory>
#include <regex>
#include <stdexcept>
#include <string>
#include <vector>

struct Pass {
    unsigned id, constants, push;
    bool compute;
    std::vector<shader_constant> values;
};

static std::vector<uint32_t> read_words(const std::filesystem::path &path) {
    std::ifstream input(path, std::ios::binary | std::ios::ate);
    const auto size = input.tellg();
    if (!input || size < 20 || size % 4) throw std::runtime_error("bad SPIR-V file: " + path.string());
    std::vector<uint32_t> words(static_cast<size_t>(size) / 4);
    input.seekg(0);
    if (!input.read(reinterpret_cast<char *>(words.data()), size)) throw std::runtime_error("short shader read");
    return words;
}

static void check(ogpu_next_status result) {
    if (result != OGPU_NEXT_OK) throw std::runtime_error("OGPU foundation status=" + std::to_string(result));
}
#define RECORD(type, kind) {kind, OGPU_NEXT_VERSION, sizeof(type), 0, nullptr}
template <class T, void (*Destroy)(T *)> using Owner = std::unique_ptr<T, decltype(Destroy)>;

int main(int argc, char **argv) {
    try {
        if (argc != 3) throw std::runtime_error("usage: check-executables capture/manifest.txt compiler-directory");
        std::ifstream input(argv[1]);
        if (!input) throw std::runtime_error("cannot read capture manifest");
        std::vector<Pass> passes;
        std::string line;
        const std::regex header(R"(^create=([0-9]+) type=(compute|raster) descriptors=[0-9]+ constants=([0-9]+) push_bytes=([0-9]+) vertex_stride=([0-9]+) vertex_attributes=([0-9]+) topology=([0-9]+) target=(none|rgba8) blend=0$)");
        const std::regex constant(R"(^  constant id=([0-9]+) type=([123]) bits=([0-9a-f]{8})$)");
        while (std::getline(input, line)) {
            std::smatch match;
            if (std::regex_match(line, match, header)) {
                const bool compute = match[2] == "compute";
                if ((!compute && (match[5] != "16" || match[6] != "2" || match[7] != "1" || match[8] != "rgba8" || match[4] != "0")) ||
                    (compute && (match[5] != "0" || match[6] != "0")))
                    throw std::runtime_error("unsupported captured executable layout");
                passes.push_back({static_cast<unsigned>(std::stoul(match[1])),
                    static_cast<unsigned>(std::stoul(match[3])), static_cast<unsigned>(std::stoul(match[4])), compute, {}});
            } else if (std::regex_match(line, match, constant)) {
                if (passes.empty()) throw std::runtime_error("constant before pass");
                passes.back().values.push_back({static_cast<uint32_t>(std::stoul(match[1])), static_cast<uint32_t>(std::stoul(match[3], nullptr, 16))});
            } else if (line.rfind("create=", 0) == 0 || line.rfind("  constant ", 0) == 0) {
                throw std::runtime_error("unsupported capture entry");
            }
        }
        if (input.bad() || passes.size() != 6) throw std::runtime_error("expected six captured passes");
        ogpu_next_discovery *raw_probe = nullptr;
        check(ogpu_next_discovery_create(OGPU_NEXT_VERSION, &raw_probe));
        Owner<ogpu_next_discovery, ogpu_next_discovery_destroy> probe(raw_probe, ogpu_next_discovery_destroy);
        if (!ogpu_next_discovery_count(raw_probe)) throw std::runtime_error("no OGPU adapter");
        const auto *adapter = ogpu_next_discovery_adapter(raw_probe, 0);
        ogpu_next_query q{RECORD(ogpu_next_query, OGPU_NEXT_QUERY_QUEUES), nullptr, 0, 0, sizeof(ogpu_next_queue_info), 0};
        check(ogpu_next_adapter_query(adapter, &q));
        std::vector<ogpu_next_queue_info> queues(q.count);
        q.data = queues.data(); q.capacity = q.count; check(ogpu_next_adapter_query(adapter, &q));
        uint32_t domain = UINT32_MAX;
        for (const auto &queue : queues)
            if (queue.count && (queue.flags & (OGPU_NEXT_QUEUE_GRAPHICS | OGPU_NEXT_QUEUE_COMPUTE)) ==
                (OGPU_NEXT_QUEUE_GRAPHICS | OGPU_NEXT_QUEUE_COMPUTE)) { domain = queue.domain; break; }
        if (domain == UINT32_MAX) throw std::runtime_error("combined graphics/compute queue required");
        const ogpu_next_queue_request request{domain, 1, .5f};
        const ogpu_next_device_desc dd{RECORD(ogpu_next_device_desc, OGPU_NEXT_DEVICE_DESC), &request, 1, 0, OGPU_NEXT_FEATURE_RASTER};
        ogpu_next_device *raw_device = nullptr;
        check(ogpu_next_device_create(adapter, &dd, &raw_device));
        Owner<ogpu_next_device, ogpu_next_device_destroy> device(raw_device, ogpu_next_device_destroy);
        for (size_t i = 0; i < passes.size(); ++i) {
            const auto &pass = passes[i];
            if (pass.id != i || pass.constants != pass.values.size()) throw std::runtime_error("incomplete or reordered pass constants");
            const auto stem = std::filesystem::path(argv[2]) / ("pass-" + std::to_string(pass.id));
            auto primary = read_words(stem.string() + (pass.compute ? ".comp.spv" : ".frag.spv"));
            const auto metadata = pass.compute ? compute_metadata(primary.data(), primary.size(), pass.values) : shader_metadata{};
            ogpu_next_shader_requirements requirements{RECORD(ogpu_next_shader_requirements, OGPU_NEXT_SHADER_REQUIREMENTS),
                0, {metadata.local[0], metadata.local[1], metadata.local[2]}, metadata.shared};
            const ogpu_next_argument_interface interface{RECORD(ogpu_next_argument_interface, OGPU_NEXT_ARGUMENT_INTERFACE),
                pass.compute ? pass.push : 8, 0, nullptr};
            std::vector<ogpu_next_specialization_entry> entries;
            std::vector<uint32_t> bits;
            for (const auto &value : pass.values) {
                entries.push_back({value.id, static_cast<uint32_t>(bits.size() * 4), 4}); bits.push_back(value.bits);
            }
            const ogpu_next_specialization specialization{RECORD(ogpu_next_specialization, OGPU_NEXT_SPECIALIZATION),
                static_cast<uint32_t>(entries.size()), 0, entries.data(), {bits.data(), bits.size() * 4}};
            std::vector<uint32_t> vertex;
            if (!pass.compute) vertex = read_words(stem.string() + ".vert.spv");
            ogpu_next_shader shaders[] = {
                {static_cast<uint32_t>(pass.compute ? OGPU_NEXT_STAGE_COMPUTE : OGPU_NEXT_STAGE_FRAGMENT), OGPU_NEXT_SHADER_SPIRV,
                    {primary.data(), primary.size() * 4}, "main", &interface.header, &specialization.header, nullptr},
                {OGPU_NEXT_STAGE_VERTEX, OGPU_NEXT_SHADER_SPIRV, {vertex.data(), vertex.size() * 4}, "main",
                    &interface.header, &specialization.header, nullptr},
            };
            if (!pass.compute) std::swap(shaders[0], shaders[1]);
            const ogpu_next_color_state color{OGPU_NEXT_RGBA8_UNORM, 15, 0, 0, 0, 0, 0, 0, 0};
            ogpu_next_graphics_state graphics{};
            graphics.header = RECORD(ogpu_next_graphics_state, OGPU_NEXT_GRAPHICS_STATE);
            graphics.topology = OGPU_NEXT_TRIANGLE_STRIP; graphics.samples = 1;
            graphics.color_count = 1; graphics.colors = &color;
            const ogpu_next_executable_desc desc{RECORD(ogpu_next_executable_desc, OGPU_NEXT_EXECUTABLE_DESC),
                pass.compute ? OGPU_NEXT_EXECUTABLE_COMPUTE : OGPU_NEXT_EXECUTABLE_GRAPHICS,
                pass.compute ? 1u : 2u, shaders, pass.compute ? nullptr : &graphics.header,
                pass.compute ? UINT64_C(0) : OGPU_NEXT_DYNAMIC_VIEWPORT_SCISSOR, &requirements.header, nullptr, 0, 0};
            ogpu_next_executable *raw = nullptr;
            check(ogpu_next_executable_create(device.get(), &desc, &raw));
            Owner<ogpu_next_executable, ogpu_next_executable_destroy> executable(raw, ogpu_next_executable_destroy);
            std::cout << "prepared pass=" << pass.id << " constants=" << pass.constants
                      << " local=" << metadata.local[0] << ',' << metadata.local[1] << ',' << metadata.local[2]
                      << " shared=" << metadata.shared << " PASS\n";
        }
        std::cout << "Six upstream executables created with captured specialization; no work submitted.\n";
    } catch (const std::exception &error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
