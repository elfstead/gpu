#include "backend.h"
#include "ggml-backend.h"
#include <cstdio>
#include <stdexcept>
#include <string>
#include <sys/resource.h>
#ifdef __linux__
#include <sys/prctl.h>
#endif

static void require(bool value, const char *message) {
    if (!value)
        throw std::runtime_error(message);
}
int main(int argc, char **argv) {
    if (argc != 3 && argc != 4)
        return 2;
    try {
        const std::string mode = argv[2];
        const std::string placement = argc == 4 ? argv[3] : "device";
        require(placement == "host" || placement == "device", "bad memory placement");
        const auto memory = placement == "host" ? OgpuGgmlMemory::Host : OgpuGgmlMemory::Device;
        if (mode == "live-backend" || mode == "live-buffer") {
            const rlimit no_core{0, 0};
            require(setrlimit(RLIMIT_CORE, &no_core) == 0, "cannot disable core dumps for death test");
#ifdef __linux__
            // Piped Linux core handlers can ignore RLIMIT_CORE.
            require(prctl(PR_SET_DUMPABLE, 0) == 0, "cannot disable core dumps for death test");
#endif
            OgpuGgmlSession session(0, argv[1], memory);
            auto backend = ggml_backend_init_by_name("OGPU", nullptr);
            require(backend != nullptr, "backend creation failed");
            if (mode == "live-buffer") {
                require(ggml_backend_alloc_buffer(backend, 256) != nullptr,
                        "buffer creation failed");
                ggml_backend_free(backend);
            }
            // Intentionally violate lifetime rules; parent test expects SIGABRT.
            return 0;
        }
        require(mode == "normal", "unknown mode");
        for (int pass = 0; pass < 3; ++pass) {
            bool failed = false;
            try {
                OgpuGgmlSession bad(UINT32_MAX, argv[1], memory);
            } catch (const std::exception &) {
                failed = true;
            }
            require(failed && !ggml_backend_reg_by_name("OGPU"),
                    "failed init published registration");
            failed = false;
            try {
                OgpuGgmlSession bad(0, "/ogpu-nonexistent-shader-directory", memory);
            } catch (const std::exception &) {
                failed = true;
            }
            require(failed && !ggml_backend_reg_by_name("OGPU"),
                    "shader failure published registration");
            {
                OgpuGgmlSession session(0, argv[1], memory);
                failed = false;
                try {
                    OgpuGgmlSession duplicate(0, argv[1], memory);
                } catch (const std::exception &) {
                    failed = true;
                }
                require(failed, "duplicate registration accepted");
                auto backend = ggml_backend_init_by_name("OGPU", nullptr);
                require(backend != nullptr, "backend creation failed");
                auto buffer = ggml_backend_alloc_buffer(backend, 256);
                require(buffer != nullptr, "buffer allocation failed");
                ggml_backend_free(backend); // Buffer independently retains session state.
                ggml_backend_buffer_clear(buffer, 0x5a);
                ggml_backend_buffer_free(buffer);
            }
            require(!ggml_backend_reg_by_name("OGPU"), "registration survived session");
        }
        std::puts("scoped lifecycle PASS");
        return 0;
    } catch (const std::exception &e) {
        std::fprintf(stderr, "%s\n", e.what());
        return 1;
    }
}
