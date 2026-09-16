#!/usr/bin/env python3
"""Pinned, deliberately narrow Slang/SPIR-V -> C adapter; no runtime dependency."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import subprocess

VERSION = "2026.14.1"
HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent


def require(condition, message):
    if not condition:
        raise ValueError(message)


def run(*args):
    result = subprocess.run(args, text=True, capture_output=True)
    if result.returncode:
        raise RuntimeError(f"{args[0]} failed:\n{result.stdout}{result.stderr}")
    return result.stdout


def identifier(name):
    # Prefix all emitted identifiers so C keywords cannot collide.
    require(isinstance(name, str) and re.fullmatch(r"[A-Za-z][A-Za-z0-9_]*", name),
            "unsupported identifier")
    return "arg_" + name


def uniform_size(typ):
    sizes = typ["sizes"]
    require(len(sizes) == 1 and sizes[0]["kind"] == "uniform", "non-uniform layout")
    return sizes[0]["value"], sizes[0]["alignment"]


def inspect(reflection, assembly):
    """Check this subset, not arbitrary SPIR-V. spirv-val runs before this."""
    parameters = reflection["parameters"]
    require(len(parameters) == 1, "exactly one root required")
    parameter = parameters[0]
    require(parameter["binding"] == {"kind": "pushConstantBuffer", "index": 0},
            "root must be push constants")
    container = parameter["type"]
    require(container["kind"] == "constantBuffer", "unsupported root container")
    root = container["elementType"]
    require(root["kind"] == "struct", "root must be struct")
    require(container["elementVarLayout"]["type"] == root, "inconsistent reflected root")
    size, alignment = uniform_size(root)
    require(isinstance(size, int) and 0 < size <= 4096 and alignment in (4, 8),
            "unsupported root size/alignment")
    require(container["elementVarLayout"]["binding"]["size"] == size,
            "inconsistent root size")
    entries = reflection["entryPoints"]
    require(len(entries) == 1, "exactly one entry required")
    entry = entries[0]
    require(entry["name"] == "main" and entry["stage"] == "compute", "unsupported entry")
    local = entry["threadGroupSize"]
    require(len(local) == 3 and all(type(n) is int and 0 < n <= 1024 for n in local),
            "unsupported local dimensions")
    require(entry["bindings"] == [{"name": parameter["name"], "binding": parameter["binding"]}],
            "unexpected entry bindings")
    require(len(entry["parameters"]) == 1 and
            entry["parameters"][0].get("semanticName") == "SV_DISPATCHTHREADID",
            "only dispatch ID entry input supported")

    caps = re.findall(r"^\s*OpCapability (\w+)\s*$", assembly, re.M)
    requirements = {"Shader": "compute_queue", "PhysicalStorageBufferAddresses": "buffer_device_address",
                    "Float16": "shader_float16"}
    require(set(caps) <= requirements.keys() and {"Shader", "PhysicalStorageBufferAddresses"} <= set(caps),
            "unmapped or missing capability")
    extensions = re.findall(r'^\s*OpExtension "([^"]+)"\s*$', assembly, re.M)
    require(set(extensions) <= {"SPV_KHR_physical_storage_buffer"}, "unmapped extension")
    require("OpSpecConstant" not in assembly and "OpExecutionModeId" not in assembly,
            "specialization unsupported")
    require(not re.search(r"\b(DescriptorSet|Binding|ResourceHeapEXT|SamplerHeapEXT)\b", assembly),
            "resource bindings unsupported")
    require(re.findall(r"OpDecorate %\S+ BuiltIn (\w+)", assembly) == ["GlobalInvocationId"],
            "only global invocation ID builtin supported")
    require(re.findall(r"OpMemoryModel (\w+) (\w+)", assembly) == [("PhysicalStorageBuffer64", "GLSL450")],
            "unsupported memory model")
    native_entries = re.findall(r'OpEntryPoint (\w+) (%\S+) "([^"]+)"', assembly)
    require(len(native_entries) == 1 and native_entries[0][0] == "GLCompute" and
            native_entries[0][2] == entry["name"], "entry mismatch")
    entry_id = native_entries[0][1]
    modes = re.findall(r"^\s*OpExecutionMode (.+)$", assembly, re.M)
    require(modes == [f"{entry_id} LocalSize {' '.join(map(str, local))}"], "local size/mode mismatch")

    definitions = {}
    for line in assembly.splitlines():
        match = re.match(r"\s*(%\S+) = (.+)", line)
        if match:
            definitions[match[1]] = match[2].split()
    variables = [(key, value) for key, value in definitions.items() if value[0] == "OpVariable"]
    require(all(v[2] in ("Input", "PushConstant", "Function") for _, v in variables),
            "shared/global resources unsupported")
    require(sum(v[2] == "Input" for _, v in variables) == 1, "unexpected input interface")
    pushes = [v for _, v in variables if v[2] == "PushConstant"]
    require(len(pushes) == 1, "exactly one SPIR-V root required")
    pointer = definitions[pushes[0][1]]
    require(pointer[:2] == ["OpTypePointer", "PushConstant"], "invalid push pointer")
    struct_id = pointer[2]
    members = definitions[struct_id]
    require(members[0] == "OpTypeStruct", "invalid push struct")
    fields = root["fields"]
    require(len(members) - 1 == len(fields) and fields, "root field count mismatch")
    decorations = re.findall(rf"OpMemberDecorate {re.escape(struct_id)} (\d+) (.+)", assembly)
    require(len(decorations) == len(fields), "unexpected member decoration")
    offsets = {}
    for index, decoration in decorations:
        require(re.fullmatch(r"Offset \d+", decoration), "unsupported member decoration")
        offsets[int(index)] = int(decoration.split()[1])
    names = re.findall(rf'OpMemberName {re.escape(struct_id)} (\d+) "([^"]+)"', assembly)
    require(dict((int(i), n) for i, n in names) == {i: f["name"] for i, f in enumerate(fields)},
            "root field name mismatch")
    result, cursor, max_alignment = [], 0, 1
    for index, (field, native_id) in enumerate(zip(fields, members[1:])):
        name = identifier(field["name"])
        require(name not in [f[0] for f in result], "duplicate field")
        typ = field["type"]
        native = definitions[native_id]
        if typ["kind"] == "pointer":
            require(typ["valueType"] == "uint", "only uint pointers supported")
            require(native[:2] == ["OpTypePointer", "PhysicalStorageBuffer"] and
                    definitions[native[2]] == ["OpTypeInt", "32", "0"], "pointer type mismatch")
            c_type, width = "uint64_t", 8
        else:
            require(typ["kind"] == "scalar" and typ["scalarType"] == "uint32" and
                    native == ["OpTypeInt", "32", "0"], "scalar type mismatch/unsupported")
            c_type, width = "uint32_t", 4
        require(uniform_size(typ) == (width, width), "field size/alignment mismatch")
        binding = field["binding"]
        offset = binding["offset"]
        require(binding["kind"] == "uniform" and binding["size"] == width and
                binding["elementStride"] == 0 and offsets.get(index) == offset,
                "field offset/size mismatch")
        require(offset == (cursor + width - 1) // width * width, "unexpected C field layout")
        result.append((name, c_type, offset, width))
        cursor = offset + width
        max_alignment = max(max_alignment, width)
    require(alignment == max_alignment and size == (cursor + alignment - 1) // alignment * alignment,
            "root size/alignment mismatch")
    return result, size, alignment, local, sorted(requirements[c] for c in set(caps))


def header(reflection, assembly, binary, source):
    fields, size, alignment, local, requirements = inspect(reflection, assembly)
    words = struct.unpack(f"<{len(binary)//4}I", binary)
    require(words[0] == 0x07230203, "invalid SPIR-V header")
    lines = ["/* Generated by examples/compiler/generate.py; do not edit.",
             f" * Slang {VERSION}; source SHA-256 {hashlib.sha256(source).hexdigest()} */",
             "#ifndef OGPU_GENERATED_TRANSFORM_H", "#define OGPU_GENERATED_TRANSFORM_H",
             '#include "ogpu.h"', "typedef struct TransformArguments {"]
    cursor = 0
    for index, (name, typ, offset, width) in enumerate(fields):
        if offset > cursor:
            lines.append(f"    uint8_t padding_{index}[{offset-cursor}];")
        lines.append(f"    {typ} {name};")
        cursor = offset + width
    if size > cursor:
        lines.append(f"    uint8_t padding_tail[{size-cursor}];")
    lines += ["} TransformArguments;", f'_Static_assert(sizeof(TransformArguments) == {size}, "root size");',
              f'_Static_assert(_Alignof(TransformArguments) == {alignment}, "root alignment");']
    for name, _, offset, _ in fields:
        lines.append(f'_Static_assert(offsetof(TransformArguments, {name}) == {offset}, "{name} offset");')
    lines += [f"static const uint32_t transform_local[3] = {{{', '.join(map(str, local))}}};",
              "static const uint32_t transform_code[] = {"]
    for start in range(0, len(words), 8):
        lines.append("    " + ", ".join(f"0x{word:08x}u" for word in words[start:start+8]) + ",")
    lines += ["};", "static inline OgpuShaderDesc transform_shader(void) {",
              '    return (OgpuShaderDesc){.code = transform_code, .code_size = sizeof(transform_code),',
              '        .entry_point = "main", .format = OGPU_SHADER_SPIRV};', "}",
              "static inline int transform_compatible(const OgpuCapabilities *c, const OgpuDeviceLimits *l) {",
              "    return " + " && ".join("c->" + r for r in requirements) + " &&",
              f"        l->max_push_data_bytes >= {size} && l->max_group_invocations >= {local[0]*local[1]*local[2]} &&",
              "        " + " && ".join(f"l->max_group_size[{i}] >= {n}" for i, n in enumerate(local)) + ";",
              "}", "#endif", ""]
    return "\n".join(lines)


def compile_source(source, output_dir, compiler):
    version = subprocess.run([compiler, "-version"], check=True, text=True, capture_output=True)
    require((version.stdout + version.stderr).strip() == VERSION, f"requires Slang {VERSION}")
    output_dir.mkdir(parents=True, exist_ok=True)
    binary, reflection = output_dir / "transform.spv", output_dir / "reflection.json"
    run(compiler, str(source), "-target", "spirv", "-profile", "spirv_1_5",
        "-emit-spirv-directly", "-fvk-use-entrypoint-name", "-fvk-use-c-layout",
        "-entry", "main", "-stage", "compute", "-reflection-json", str(reflection), "-o", str(binary))
    run("spirv-val", "--target-env", "vulkan1.4", str(binary))
    return json.loads(reflection.read_text()), run("spirv-dis", str(binary)), binary.read_bytes()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--source", type=Path, default=HERE / "transform.slang")
    parser.add_argument("--output", type=Path, default=HERE / "transform.generated.h")
    parser.add_argument("--build-dir", type=Path, default=ROOT / "target/compiler-workflow")
    args = parser.parse_args()
    reflected, assembly, binary = compile_source(args.source, args.build_dir, os.getenv("SLANGC", "slangc"))
    generated = header(reflected, assembly, binary, args.source.read_bytes())
    if args.check:
        require(args.output.read_text() == generated, "stale generated interface; regenerate")
    else:
        args.output.write_text(generated)
    print("Compiler root/artifact/requirements generation PASS")


if __name__ == "__main__":
    main()
