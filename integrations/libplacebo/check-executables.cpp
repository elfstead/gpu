// G1 only: create prepared OGPU executables from captured upstream shaders and
// their exact constant bits. No dispatch/draw and no image-comparison claim.
#include "ogpu.h"
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
    std::vector<OgpuSpecializationConstant> values;
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

static void check(OgpuResult result, const OgpuError &error) {
    if (result != OGPU_SUCCESS) throw std::runtime_error("OGPU status=" + std::to_string(result) + ": " + error.message);
}

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
        OgpuError error{};
        OgpuProbe *raw_probe = nullptr;
        auto status = ogpu_probe_create(OGPU_ABI_VERSION, &raw_probe, &error);
        std::unique_ptr<OgpuProbe, decltype(&ogpu_probe_destroy)> probe(raw_probe, ogpu_probe_destroy);
        check(status, error);
        uint32_t count = 0;
        check(ogpu_probe_device_count(probe.get(), &count), error);
        OgpuDevice *raw_device = nullptr;
        for (uint32_t index = 0; index < count; ++index) {
            status = ogpu_device_create_graphics(probe.get(), index, &raw_device, &error);
            if (status == OGPU_ERROR_UNSUPPORTED) continue;
            check(status, error);
            break;
        }
        std::unique_ptr<OgpuDevice, decltype(&ogpu_device_destroy)> device(raw_device, ogpu_device_destroy);
        if (!device) throw std::runtime_error("no graphics-capable OGPU device");
        for (size_t i = 0; i < passes.size(); ++i) {
            const auto &pass = passes[i];
            if (pass.id != i || pass.constants != pass.values.size()) throw std::runtime_error("incomplete or reordered pass constants");
            const auto stem = std::filesystem::path(argv[2]) / ("pass-" + std::to_string(pass.id));
            auto primary = read_words(stem.string() + (pass.compute ? ".comp.spv" : ".frag.spv"));
            const OgpuShaderDesc shader = {primary.data(), primary.size(), pass.values.data(), static_cast<uint32_t>(pass.values.size()), 0};
            if (pass.compute) {
                OgpuKernel *raw = nullptr;
                status = ogpu_kernel_create(device.get(), &shader, pass.push, &raw, &error);
                std::unique_ptr<OgpuKernel, decltype(&ogpu_kernel_destroy)> kernel(raw, ogpu_kernel_destroy);
                check(status, error);
            } else {
                auto vertex = read_words(stem.string() + ".vert.spv");
                const OgpuShaderDesc vs = {vertex.data(), vertex.size(), pass.values.data(), static_cast<uint32_t>(pass.values.size()), 0};
                OgpuRaster *raw = nullptr;
                status = ogpu_raster_create(device.get(), &vs, &shader, 8, OGPU_TOPOLOGY_TRIANGLE_STRIP, &raw, &error);
                std::unique_ptr<OgpuRaster, decltype(&ogpu_raster_destroy)> raster(raw, ogpu_raster_destroy);
                check(status, error);
            }
            std::cout << "prepared pass=" << pass.id << " constants=" << pass.constants << " PASS\n";
        }
        std::cout << "Six upstream executables created with captured specialization; no work submitted.\n";
    } catch (const std::exception &error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
