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
import sys

VERSION = "2026.14.1"
HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE))
import layouts


def require(condition, message):
    if not condition:
        raise ValueError(message)


def run(*args):
    result = subprocess.run(args, text=True, capture_output=True)
    if result.returncode:
        raise RuntimeError(f"{args[0]} failed:\n{result.stdout}{result.stderr}")
    return result.stdout


def uniform_size(typ):
    sizes = typ["sizes"]
    require(len(sizes) == 1 and sizes[0]["kind"] == "uniform", "non-uniform layout")
    return sizes[0]["value"], sizes[0]["alignment"]


def reflected_type(typ):
    if typ["kind"] == "scalar":
        return (typ["scalarType"], 1)
    require(typ["kind"] == "vector" and typ["elementType"]["kind"] == "scalar",
            "unsupported entry interface type")
    return (typ["elementType"]["scalarType"], typ["elementCount"])


def inspect(reflection, assembly, name="transform"):
    """Check this subset, not arbitrary SPIR-V. spirv-val runs before this."""
    entries = reflection["entryPoints"]
    require(len(entries) == 1, "exactly one entry required")
    entry = entries[0]
    stage = entry["stage"]
    require(entry["name"] == "main" and stage in ("compute", "vertex", "fragment"), "unsupported entry")
    local = entry.get("threadGroupSize", [])
    if stage == "compute":
        require(len(local) == 3 and all(type(n) is int and 0 < n <= 1024 for n in local),
                "unsupported local dimensions")
    else:
        require(not local, "graphics entry cannot declare local dimensions")
    parameters = reflection["parameters"]
    require(len(parameters) == (0 if stage == "vertex" else 1), "unexpected root count")
    require(entry["bindings"] == [{"name": p["name"], "binding": p["binding"]} for p in parameters],
            "unexpected entry bindings")
    semantic, scalar, lanes, builtin = {
        "compute": ("SV_DISPATCHTHREADID", "uint32", 3, "GlobalInvocationId"),
        "vertex": ("SV_VULKANVERTEXID", "uint32", 1, "VertexIndex"),
        "fragment": ("SV_POSITION", "float32", 4, "FragCoord"),
    }[stage]
    require(len(entry["parameters"]) == 1 and entry["parameters"][0].get("semanticName") == semantic
            and reflected_type(entry["parameters"][0]["type"]) == (scalar, lanes),
            "unsupported entry input")
    if stage == "compute":
        require("result" not in entry, "compute output unsupported")
    else:
        result = entry["result"]
        require(result["semanticName"] == ("SV_POSITION" if stage == "vertex" else "SV_TARGET")
                and reflected_type(result["type"]) == ("float32", 4), "unsupported entry output")
        require(result.get("binding") == (None if stage == "vertex" else {"kind": "varyingOutput", "index": 0}),
                "unsupported output binding")

    caps = re.findall(r"^\s*OpCapability (\w+)\s*$", assembly, re.M)
    requirements = {"Shader": "compute_queue" if stage == "compute" else "graphics_queue",
                    "PhysicalStorageBufferAddresses": "buffer_device_address",
                    "Float16": "shader_float16"}
    require(set(caps) <= requirements.keys() and "Shader" in caps,
            "unmapped or missing capability")
    extensions = re.findall(r'^\s*OpExtension "([^"]+)"\s*$', assembly, re.M)
    require(set(extensions) <= {"SPV_KHR_physical_storage_buffer"}, "unmapped extension")
    require("OpSpecConstant" not in assembly and "OpExecutionModeId" not in assembly,
            "specialization unsupported")
    require(not re.search(r"\b(DescriptorSet|Binding|ResourceHeapEXT|SamplerHeapEXT)\b", assembly),
            "resource bindings unsupported")
    physical = "PhysicalStorageBufferAddresses" in caps
    require(re.findall(r"OpMemoryModel (\w+) (\w+)", assembly) ==
            [("PhysicalStorageBuffer64" if physical else "Logical", "GLSL450")], "unsupported memory model")
    native_entries = re.findall(r'^\s*OpEntryPoint (\w+) (%\S+) "([^"]+)"(.*)$', assembly, re.M)
    require(len(native_entries) == 1 and native_entries[0][0] ==
            {"compute": "GLCompute", "vertex": "Vertex", "fragment": "Fragment"}[stage] and
            native_entries[0][2] == entry["name"], "entry mismatch")
    entry_id = native_entries[0][1]
    modes = re.findall(r"^\s*OpExecutionMode (.+)$", assembly, re.M)
    expected_modes = ([f"{entry_id} LocalSize {' '.join(map(str, local))}"] if stage == "compute" else
                      [f"{entry_id} OriginUpperLeft"] if stage == "fragment" else [])
    require(modes == expected_modes, "local size/mode mismatch")

    definitions = {}
    for line in assembly.splitlines():
        match = re.match(r"\s*(%\S+) = (.+)", line)
        if match:
            definitions[match[1]] = match[2].split()
    check_push_indices(definitions)
    variables = [(key, value) for key, value in definitions.items() if value[0] == "OpVariable"]
    require(all(v[2] in ("Input", "Output", "PushConstant", "Function") for _, v in variables),
            "shared/global resources unsupported")
    interfaces = native_entries[0][3].split()
    require(len(interfaces) == len(set(interfaces)) and set(interfaces) ==
            {key for key, v in variables if v[2] != "Function"}, "entry variable list mismatch")
    expected_interface = [("Input", f"BuiltIn {builtin}", "int32" if stage == "vertex" else scalar, lanes)]
    if stage != "compute":
        expected_interface.append(("Output", "BuiltIn Position" if stage == "vertex" else "Location 0", "float32", 4))
    actual_interface = []
    interface_ids = set()
    for key, variable in variables:
        if variable[2] not in ("Input", "Output"):
            continue
        interface_ids.add(key)
        pointer = definitions[variable[1]]
        require(pointer[:2] == ["OpTypePointer", variable[2]], "invalid interface pointer")
        typ = definitions[pointer[2]]
        count = 1
        if typ[0] == "OpTypeVector":
            count = int(typ[2])
            typ = definitions[typ[1]]
        scalar_name = {("OpTypeFloat", "32"): "float32", ("OpTypeInt", "32", "0"): "uint32",
                       ("OpTypeInt", "32", "1"): "int32"}.get(tuple(typ))
        decorations = re.findall(rf"^\s*OpDecorate {re.escape(key)} (.+)$", assembly, re.M)
        require(len(decorations) == 1, "unsupported interface decorations")
        actual_interface.append((variable[2], decorations[0], scalar_name, count))
    require(sorted(actual_interface) == sorted(expected_interface), "native entry interface mismatch")
    # No hidden/member builtins or extra locations outside the checked variables.
    decorated_io = re.findall(r"OpDecorate (%\S+) (?:BuiltIn|Location) \w+", assembly)
    require(set(decorated_io) == interface_ids and len(decorated_io) == len(interface_ids), "extra interface decoration")
    require(not re.search(r"OpMemberDecorate %\S+ \d+ (BuiltIn|Location)\b", assembly), "member interfaces unsupported")
    pushes = [v for _, v in variables if v[2] == "PushConstant"]
    if stage == "vertex":
        require(not pushes and not physical, "vertex must be rootless and address-free")
        return [], 0, 1, [], sorted(requirements[c] for c in set(caps))
    require(len(pushes) == 1, "exactly one SPIR-V root required")
    parameter = parameters[0]
    require(parameter["binding"] == {"kind": "pushConstantBuffer", "index": 0}, "root must be push constants")
    container = parameter["type"]
    require(container["kind"] == "constantBuffer", "unsupported root container")
    root = container["elementType"]
    require(root["kind"] == "struct", "root must be struct")
    require(container["elementVarLayout"]["type"] == root, "inconsistent reflected root")
    size, alignment = uniform_size(root)
    require(isinstance(size, int) and 0 < size <= 4096 and alignment in (4, 8), "unsupported root size/alignment")
    require(container["elementVarLayout"]["binding"]["size"] == size, "inconsistent root size")
    pointer = definitions[pushes[0][1]]
    require(pointer[:2] == ["OpTypePointer", "PushConstant"], "invalid push pointer")
    struct_id = pointer[2]
    result, checked_size, checked_alignment = layouts.inspect_root(
        root, struct_id, definitions, assembly, reflection.get("ogpuPointeeLayouts", {}), physical, name)
    require((size, alignment) == (checked_size, checked_alignment), "root size/alignment mismatch")
    return result, size, alignment, local, sorted(requirements[c] for c in set(caps))


def header(reflection, assembly, binary, source, name="transform"):
    require(re.fullmatch(r"[a-z][a-z0-9_]*", name), "unsupported artifact name")
    fields, size, alignment, local, requirements = inspect(reflection, assembly, name)
    stage = reflection["entryPoints"][0]["stage"]
    type_name = name[0].upper() + name[1:] + "Arguments"
    guard = "OGPU_GENERATED_" + name.upper() + "_H"
    require(len(binary) >= 20 and len(binary) % 4 == 0, "invalid SPIR-V size")
    words = struct.unpack(f"<{len(binary)//4}I", binary)
    require(words[0] == 0x07230203, "invalid SPIR-V header")
    lines = ["/* Generated by examples/compiler/generate.py; do not edit.",
             f" * Slang {VERSION}; source SHA-256 {hashlib.sha256(source).hexdigest()} */",
             f"#ifndef {guard}", f"#define {guard}", '#include "ogpu.h"',
             f"/* Stage tags are generator-local: compute=0, vertex=1, fragment=2. */",
             f"enum {{ {name}_stage = {['compute', 'vertex', 'fragment'].index(stage)}, {name}_push_size = {size} }};"]
    if getattr(fields, "floating", False):
        lines += ['#include <float.h>',
                  '_Static_assert(sizeof(float) == 4 && _Alignof(float) == 4, "FP32 storage/alignment");',
                  '_Static_assert(FLT_RADIX == 2 && FLT_MANT_DIG == 24 && FLT_MAX_EXP == 128, "FP32 representation");']
    lines += getattr(fields, "declarations", [])
    if fields:
        lines += layouts.struct_lines(type_name, fields, size, alignment, root=True)
    if local:
        lines.append(f"static const uint32_t {name}_local[3] = {{{', '.join(map(str, local))}}};")
    lines.append(f"static const uint32_t {name}_code[] = {{")
    for start in range(0, len(words), 8):
        lines.append("    " + ", ".join(f"0x{word:08x}u" for word in words[start:start+8]) + ",")
    lines += ["};", f"static inline OgpuShaderDesc {name}_shader(void) {{",
              f'    return (OgpuShaderDesc){{.code = {name}_code, .code_size = sizeof({name}_code),',
              '        .entry_point = "main", .format = OGPU_SHADER_SPIRV};', "}",
              f"static inline int {name}_compatible(const OgpuCapabilities *c, const OgpuDeviceLimits *l) {{"]
    checks = ["c->" + r for r in requirements]
    if size:
        checks.append(f"l->max_push_data_bytes >= {size}")
    if local:
        checks += [f"l->max_group_invocations >= {local[0]*local[1]*local[2]}"]
        checks += [f"l->max_group_size[{i}] >= {n}" for i, n in enumerate(local)]
    if not size and not local:
        lines.append("    (void)l;")
    lines += ["    return " + " && ".join(checks) + ";", "}", "#endif", ""]
    return "\n".join(lines)


def compile_source(source, output_dir, compiler, stage="compute"):
    require(stage in ("compute", "vertex", "fragment"), "unsupported compilation stage")
    version = subprocess.run([compiler, "-version"], check=True, text=True, capture_output=True)
    require((version.stdout + version.stderr).strip() == VERSION, f"requires Slang {VERSION}")
    output_dir.mkdir(parents=True, exist_ok=True)
    binary, reflection = output_dir / "transform.spv", output_dir / "reflection.json"
    options = ["-target", "spirv", "-profile", "spirv_1_5", "-emit-spirv-directly",
               "-fvk-use-entrypoint-name", "-fvk-use-c-layout", "-entry", "main", "-stage", stage]
    run(compiler, str(source), *options, "-reflection-json", str(reflection), "-o", str(binary))
    run("spirv-val", "--target-env", "vulkan1.4", str(binary))
    reflected = json.loads(reflection.read_text())
    pointees = {}
    pending = named_pointees(reflected)
    # Query C-layout metadata separately. Never ship this query's extra parameters
    # or substitute its output for the validated, original shader artifact.
    while pending - pointees.keys():
        require(len(pending) <= 32, "too many named pointee layouts")
        for name in pending: layouts.identifier(name)
        query = output_dir / "layout-query.slang"
        query.write_text("#include " + json.dumps(str(source.resolve())) + "\n" + "\n".join(
            f"[[vk::push_constant]] ConstantBuffer<{name}> ogpuLayoutQuery_{name};" for name in sorted(pending)) + "\n")
        metadata = output_dir / "layout-query.json"
        run(compiler, str(query), *options, "-no-codegen", "-reflection-json", str(metadata))
        parameters = json.loads(metadata.read_text())["parameters"]
        for parameter in reflected["parameters"]:
            require([p for p in parameters if p["name"] == parameter["name"]] == [parameter],
                    "layout query changed original parameter metadata")
        for name in sorted(pending):
            selected = [p for p in parameters if p["name"] == "ogpuLayoutQuery_" + name]
            require(len(selected) == 1, "missing/ambiguous pointee layout query")
            container = selected[0]["type"]
            typ = container["elementType"]
            require(container["kind"] == "constantBuffer" and typ["kind"] == "struct" and typ["name"] == name
                    and container["elementVarLayout"]["type"] == typ
                    and container["elementVarLayout"]["binding"]["size"] == uniform_size(typ)[0],
                    "inconsistent pointee layout query")
            if name in pointees: require(pointees[name] == typ, "pointee layout changed between queries")
            pointees[name] = typ
        pending |= named_pointees(pointees)
    if pointees:
        reflected["ogpuPointeeLayouts"] = pointees
        (output_dir / "interface.json").write_text(json.dumps(reflected, indent=2) + "\n")
    return reflected, run("spirv-dis", str(binary)), binary.read_bytes()


def named_pointees(value):
    result = set()
    if isinstance(value, dict):
        if value.get("kind") == "pointer" and value["valueType"] not in ("uint", "float"):
            result.add(value["valueType"])
        for child in value.values(): result |= named_pointees(child)
    elif isinstance(value, list):
        for child in value: result |= named_pointees(child)
    return result


def check_push_indices(definitions):
    """Conservative proof for this profile, not general shader uniformity analysis.

    Vulkan requires dynamically uniform push-array indices. spirv-val does not
    establish runtime uniformity. Unknown data flow rejects rather than guesses;
    legal native programs outside this proof remain a compiler-adapter limitation.
    """
    active = set()
    known = {}
    arithmetic = {"OpIAdd", "OpISub", "OpIMul", "OpUDiv", "OpSDiv", "OpUMod", "OpSMod", "OpSRem",
                  "OpBitwiseAnd", "OpBitwiseOr", "OpBitwiseXor", "OpShiftLeftLogical", "OpShiftRightLogical",
                  "OpShiftRightArithmetic", "OpIEqual", "OpINotEqual", "OpULessThan", "OpUGreaterThan",
                  "OpSLessThan", "OpSGreaterThan", "OpLogicalAnd", "OpLogicalOr", "OpSelect"}
    chains = {"OpAccessChain", "OpInBoundsAccessChain", "OpPtrAccessChain", "OpInBoundsPtrAccessChain"}
    def uniform(identity):
        if identity in known: return known[identity]
        if identity in active: return False
        active.add(identity)
        value = definitions.get(identity, [])
        if not value: okay = False
        elif value[0] in ("OpConstant", "OpConstantTrue", "OpConstantFalse", "OpConstantNull"):
            okay = True
        elif value[0] == "OpVariable": okay = value[2] == "PushConstant"
        elif value[0] in chains or value[0] in arithmetic or value[0] in ("OpConstantComposite", "OpCompositeConstruct"):
            okay = all(uniform(operand) for operand in value[2:])
        elif value[0] in ("OpCopyObject", "OpBitcast", "OpUConvert", "OpSConvert", "OpCompositeExtract"):
            okay = uniform(value[2])
        elif value[0] == "OpLoad":
            pointer = definitions.get(value[2], [])
            typ = definitions.get(pointer[1], []) if len(pointer) > 1 else []
            okay = typ[:2] == ["OpTypePointer", "PushConstant"] and uniform(value[2])
        else: okay = False
        active.remove(identity); known[identity] = okay
        return okay
    for value in definitions.values():
        if value and value[0] in chains and definitions.get(value[1], [])[:2] == ["OpTypePointer", "PushConstant"]:
            require(all(uniform(index) for index in value[3:]),
                    "push-array index uniformity unproven; use constant/uniform root indices or pointed-to data")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--source", type=Path, default=HERE / "transform.slang")
    parser.add_argument("--output", type=Path, default=HERE / "transform.generated.h")
    parser.add_argument("--build-dir", type=Path, default=ROOT / "target/compiler-workflow")
    parser.add_argument("--name", default="transform")
    parser.add_argument("--stage", choices=("compute", "vertex", "fragment"), default="compute")
    args = parser.parse_args()
    reflected, assembly, binary = compile_source(args.source, args.build_dir, os.getenv("SLANGC", "slangc"), args.stage)
    generated = header(reflected, assembly, binary, args.source.read_bytes(), args.name)
    if args.check:
        require(args.output.read_text() == generated, "stale generated interface; regenerate")
    else:
        args.output.write_text(generated)
    print("Compiler root/artifact/requirements generation PASS")


if __name__ == "__main__":
    main()
