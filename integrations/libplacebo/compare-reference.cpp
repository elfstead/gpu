// Compare fixed workload outputs. --consumer checks both intermediate and final
// images against upstream on the same driver, using the predeclared G2 tolerance.
#include <algorithm>
#include <fstream>
#include <iostream>
#include <iterator>
#include <stdexcept>
#include <string>
#include <vector>

static std::vector<unsigned char> read(const std::string &path, size_t expected) {
    std::ifstream in(path, std::ios::binary);
    if (!in)
        throw std::runtime_error("cannot open " + path);
    std::vector<unsigned char> bytes{std::istreambuf_iterator<char>(in), {}};
    if (in.bad() || bytes.size() != expected)
        throw std::runtime_error("invalid size or read failure: " + path);
    return bytes;
}

int main(int argc, char **argv) {
    try {
        const bool consumer = argc == 4 && std::string(argv[3]) == "--consumer";
        const bool stream = argc == 4 && std::string(argv[3]) == "--stream";
        if (argc != 3 && !consumer && !stream)
            throw std::runtime_error("usage: compare-reference capture-a capture-b [--consumer|--stream]");
        const int widths[] = {16, 31, 64}, heights[] = {16, 17, 33};
        unsigned maximum = 0;
        size_t differences = 0, total = 0;
        for (unsigned c = 0; c < 3; ++c) {
            for (unsigned frame = 0; frame < (stream ? 12u : 3u); ++frame) {
                for (unsigned stage = 0; stage < (consumer || stream ? 2u : 1u); ++stage) {
                    const auto prefix = "/pass-" + std::to_string(2 * (c + 1)) + "." +
                        std::to_string(widths[c]) + "x" + std::to_string(heights[c]) +
                        "-frame";
                    const auto suffix = stage ? "-middle.rgba" : ".rgba";
                    const auto file = prefix + std::to_string(frame) + suffix;
                    const auto reference = prefix + std::to_string(stream ? frame % 3 : frame) + suffix;
                    const size_t size = (widths[c] * 2 + 1) * (heights[c] * 2 + 1) * 4;
                    const auto a = read(argv[1] + reference, size), b = read(argv[2] + file, size);
                    for (size_t i = 0; i < size; ++i) {
                        const unsigned delta = a[i] > b[i] ? a[i] - b[i] : b[i] - a[i];
                        maximum = std::max(maximum, delta);
                        differences += delta != 0;
                        ++total;
                        if ((i % 4 == 3 && (a[i] != 255 || b[i] != 255)) || delta > 2)
                            throw std::runtime_error("tolerance exceeded: " + file + " byte=" + std::to_string(i));
                    }
                }
            }
        }
        std::cout << (consumer || stream ? "OGPU/reference intermediate+final" : "reference")
                  << " comparison bytes=" << total << " differing=" << differences
                  << " max_byte_delta=" << maximum << " PASS\n";
    } catch (const std::exception &e) {
        std::cerr << e.what() << '\n';
        return 1;
    }
}
