// Compare the same fixed reference workload across drivers. This is not a
// substitute for comparing OGPU against upstream on each driver in G2.
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
        if (argc != 3)
            throw std::runtime_error("usage: compare-reference capture-a capture-b");
        const int widths[] = {16, 31, 64}, heights[] = {16, 17, 33};
        unsigned maximum = 0;
        size_t differences = 0, total = 0;
        for (unsigned c = 0; c < 3; ++c) {
            for (unsigned frame = 0; frame < 3; ++frame) {
                const auto file = "/pass-" + std::to_string(2 * (c + 1)) + "." +
                    std::to_string(widths[c]) + "x" + std::to_string(heights[c]) +
                    "-frame" + std::to_string(frame) + ".rgba";
                const size_t size = (widths[c] * 2 + 1) * (heights[c] * 2 + 1) * 4;
                const auto a = read(argv[1] + file, size), b = read(argv[2] + file, size);
                for (size_t i = 0; i < size; ++i) {
                    const unsigned delta = a[i] > b[i] ? a[i] - b[i] : b[i] - a[i];
                    maximum = std::max(maximum, delta);
                    differences += delta != 0;
                    ++total;
                    if ((i % 4 == 3 && (a[i] != 255 || b[i] != 255)) || delta > 2)
                        throw std::runtime_error("reference tolerance exceeded");
                }
            }
        }
        std::cout << "reference comparison bytes=" << total << " differing=" << differences
                  << " max_byte_delta=" << maximum << " PASS\n";
    } catch (const std::exception &e) {
        std::cerr << e.what() << '\n';
        return 1;
    }
}
