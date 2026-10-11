#pragma once
// Preparation-only consumer reflection. The foundation runtime never sees this
// parser or the specialized analysis copy. Native execution keeps the original
// binary and specialization map, including float bit patterns.
#include <spirv-tools/optimizer.hpp>
#include <cstdint>
#include <limits>
#include <stdexcept>
#include <string>
#include <unordered_map>
#include <vector>

struct shader_constant { uint32_t id, bits; };
struct shader_metadata { uint32_t local[3]{}, shared = 0; };

inline shader_metadata compute_metadata(const uint32_t *code, size_t words,
                                       const std::vector<shader_constant> &values) {
    std::unordered_map<uint32_t, std::vector<uint32_t>> constants;
    for (const auto &value : values)
        if (!constants.emplace(value.id, std::vector<uint32_t>{value.bits}).second)
            throw std::runtime_error("duplicate specialization ID");
    spvtools::Optimizer optimizer(SPV_ENV_VULKAN_1_4);
    std::string diagnostic;
    optimizer.SetMessageConsumer([&](spv_message_level_t, const char *, const spv_position_t &, const char *message) {
        diagnostic += message; diagnostic += '\n';
    });
    optimizer.RegisterPass(spvtools::CreateSetSpecConstantDefaultValuePass(constants));
    optimizer.RegisterPass(spvtools::CreateFreezeSpecConstantValuePass());
    optimizer.RegisterPass(spvtools::CreateFoldSpecConstantOpAndCompositePass());
    std::vector<uint32_t> specialized;
    if (!optimizer.Run(code, words, &specialized))
        throw std::runtime_error("shader metadata specialization failed: " + diagnostic);

    // Intentionally bounded to this adapter's compiler output: one compute main,
    // 32-bit scalars, vec2/vec4, and fixed arrays for Workgroup storage. Unknown
    // storage layouts fail rather than advertise invented shared-memory usage.
    enum : uint32_t { EntryPoint=15, ExecutionMode=16, Capability=17, TypeInt=21, TypeFloat=22,
        TypeVector=23, TypeArray=28, TypePointer=32, Constant=43, Variable=59, Decorate=71,
        ExecutionModeId=331, LocalSize=17, LocalSizeId=38, Workgroup=4, Compute=5 };
    struct instruction { uint32_t op; std::vector<uint32_t> args; };
    std::unordered_map<uint32_t, instruction> definitions;
    std::vector<uint32_t> shared_types;
    shader_metadata result;
    uint32_t entry = 0, mode_entry = 0, mode = 0, dimensions[3]{};
    for (size_t i = 5; i < specialized.size();) {
        uint32_t count = specialized[i] >> 16, op = specialized[i] & 0xffff;
        if (!count || count > specialized.size() - i) throw std::runtime_error("bad SPIR-V instruction");
        const auto *a = specialized.data() + i + 1;
        auto need = [&](uint32_t n) { if (count < n + 1) throw std::runtime_error("short SPIR-V instruction"); };
        switch (op) {
        case Capability:
            need(1);
            if (a[0] >= 4428 && a[0] <= 4430)
                throw std::runtime_error("explicit shared-memory layout is outside this metadata profile");
            break;
        case Decorate:
            need(2);
            if (a[1] == 11) { // BuiltIn; legacy WorkgroupSize overrides LocalSize.
                need(3);
                if (a[2] == 25) throw std::runtime_error("legacy WorkgroupSize metadata is unsupported");
            }
            break;
        case EntryPoint:
            need(4);
            if (entry || a[0] != Compute || a[2] != 0x6e69616d || a[3] != 0)
                throw std::runtime_error("metadata requires one compute main");
            entry = a[1]; break;
        case ExecutionMode: case ExecutionModeId:
            need(2);
            if (a[1] == LocalSize || a[1] == LocalSizeId) {
                need(5);
                if (mode || (a[1] == LocalSizeId) != (op == ExecutionModeId))
                    throw std::runtime_error("ambiguous local size");
                mode = a[1]; mode_entry = a[0];
                for (unsigned d = 0; d < 3; ++d) dimensions[d] = a[d+2];
            }
            break;
        case TypeInt: case TypeFloat: case TypeVector: case TypeArray: case TypePointer:
            need(2);
            definitions.emplace(a[0], instruction{op, {a+1, a+count-1}}); break;
        case Constant:
            need(3);
            definitions.emplace(a[1], instruction{op, {a[0], a[2]}}); break;
        case Variable:
            need(3);
            if (a[2] == Workgroup) shared_types.push_back(a[0]);
            break;
        default: break;
        }
        i += count;
    }
    auto get = [&](uint32_t id) -> const instruction & {
        auto found = definitions.find(id);
        if (found == definitions.end()) throw std::runtime_error("unsupported metadata dependency");
        return found->second;
    };
    auto scalar = [&](uint32_t id) {
        const auto &v = get(id);
        if (v.op != Constant || v.args.size() != 2) throw std::runtime_error("unresolved metadata constant");
        const auto &type = get(v.args[0]);
        if (type.op != TypeInt || type.args.size() != 2 || type.args[0] != 32 ||
            (type.args[1] && (v.args[1] & 0x80000000u)))
            throw std::runtime_error("unsupported or negative metadata constant");
        return v.args[1];
    };
    if (!entry || !mode || mode_entry != entry) throw std::runtime_error("missing local size");
    for (unsigned d = 0; d < 3; ++d) {
        result.local[d] = mode == LocalSize ? dimensions[d] : scalar(dimensions[d]);
        if (!result.local[d]) throw std::runtime_error("zero local size");
    }
    auto size_of = [&](auto &&self, uint32_t id, unsigned depth) -> uint64_t {
        if (depth > 16) throw std::runtime_error("excessive metadata type nesting");
        const auto &type = get(id);
        const auto &a = type.args;
        uint64_t bytes = 0;
        if ((type.op == TypeInt || type.op == TypeFloat) && a[0] == 32) bytes = 4;
        else if (type.op == TypeVector && a.size() == 2 && (a[1] == 2 || a[1] == 4)) {
            const auto &element = get(a[0]);
            if (element.op != TypeInt && element.op != TypeFloat) throw std::runtime_error("unsupported vector element");
            bytes = self(self, a[0], depth+1) * a[1];
        } else if (type.op == TypeArray && a.size() == 2) {
            bytes = self(self, a[0], depth+1) * scalar(a[1]);
        } else throw std::runtime_error("unsupported shared-memory type");
        if (!bytes || bytes > std::numeric_limits<uint32_t>::max()) throw std::runtime_error("shared-memory size overflow");
        return bytes;
    };
    uint64_t total = 0;
    for (uint32_t id : shared_types) {
        const auto &pointer = get(id);
        if (pointer.op != TypePointer || pointer.args.size() != 2 || pointer.args[0] != Workgroup)
            throw std::runtime_error("unsupported shared-memory pointer");
        total += size_of(size_of, pointer.args[1], 0);
        if (total > std::numeric_limits<uint32_t>::max()) throw std::runtime_error("total shared-memory size overflow");
    }
    result.shared = static_cast<uint32_t>(total);
    return result;
}
