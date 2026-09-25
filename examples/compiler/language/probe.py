#!/usr/bin/env python3
"""M2 language diagnostics, NOT an extension of the installed shader adapter.

Compile and inspect three fixed probes. --gpu additionally uses the existing
public reduction/matrix consumers and a capability-queried native subgroup oracle.
This checks a bounded lowering, not arbitrary shader safety or machine-code cost.
"""
import argparse
from collections import Counter
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
import generate
import layouts
import link_graphics

ROOT = generate.ROOT
require = generate.require


def definitions(assembly):
    return {m[1]: m[2].split() for m in re.finditer(r"^\s*(%\S+) = (.+)$", assembly, re.M)}


def constant(native, identifier):
    value = native[identifier]
    require(value[0] == "OpConstant" and native[value[1]] in
            (["OpTypeInt", "32", "0"], ["OpTypeInt", "32", "1"]), "non-integer constant")
    return int(value[2])


def barriers(assembly):
    native = definitions(assembly)
    return [[constant(native, v) for v in row.split()] for row in
            re.findall(r"^\s*OpControlBarrier (.+)$", assembly, re.M)]


def inspect(name, reflection, assembly):
    """Exact probe expectations independent of native IDs and instruction order."""
    native = definitions(assembly)
    caps = sorted(re.findall(r"^\s*OpCapability (\w+)$", assembly, re.M))
    expected_caps = ["PhysicalStorageBufferAddresses", "Shader"]
    if name == "subgroup": expected_caps += ["GroupNonUniform", "GroupNonUniformArithmetic"]
    require(caps == sorted(expected_caps), "unexpected capability")
    require(re.findall(r'^\s*OpExtension "([^"]+)"$', assembly, re.M) ==
            ["SPV_KHR_physical_storage_buffer"], "unexpected extension")
    require(re.findall(r"OpMemoryModel (\w+) (\w+)", assembly) ==
            [("PhysicalStorageBuffer64", "GLSL450")], "unexpected memory model")
    entries = re.findall(r'^\s*OpEntryPoint GLCompute (%\S+) "main" (.+)$', assembly, re.M)
    require(len(entries) == 1 and assembly.count("OpEntryPoint") == 1, "unexpected entry")
    require(re.findall(r"^\s*OpExecutionMode (.+)$", assembly, re.M) ==
            [f"{entries[0][0]} LocalSize 64 1 1"], "unexpected local size")
    require(len(reflection["entryPoints"]) == 1 and
            reflection["entryPoints"][0]["threadGroupSize"] == [64, 1, 1], "reflected local size")
    require(not re.search(r"\b(DescriptorSet|Binding|OpCopyMemory\w*|OpFunctionCall|OpMemoryBarrier|OpSpecConstant\w*|OpExecutionModeId)\b",
                          assembly), "unexpected binding/copy/call/barrier/specialization")
    variables = [v for v in native.values() if v[0] == "OpVariable"]
    require(all(v[2] in ("Input", "PushConstant", "Function", "Workgroup") for v in variables),
            "unexpected storage")
    push = [v for v in variables if v[2] == "PushConstant"]
    require(len(push) == 1 and len(reflection["parameters"]) == 1, "unexpected root count")
    parameter = reflection["parameters"][0]
    require(parameter["binding"] == {"kind": "pushConstantBuffer", "index": 0}, "unexpected root binding")
    container = parameter["type"]
    root = container["elementType"]
    require(container["kind"] == "constantBuffer" and container["elementVarLayout"]["type"] == root,
            "inconsistent root reflection")
    pointer = native[push[0][1]]
    require(pointer[:2] == ["OpTypePointer", "PushConstant"], "unexpected root pointer")
    fields, size, alignment = layouts.inspect_root(root, pointer[2], native, assembly, {}, True, name)
    expected = ([("a", "uint64_t", 0, 8), ("b", "uint64_t", 8, 8), ("c", "uint64_t", 16, 8)] +
                [(f, "uint32_t", 24+4*i, 4) for i, f in enumerate(("m", "n", "k", "lda", "ldb", "ldc"))]
                if name == "matmul" else [("input_data", "uint64_t", 0, 8),
                                          ("output_data", "uint64_t", 8, 8), ("count", "uint32_t", 16, 4)])
    require(list(fields) == [("arg_"+f, t, o, w) for f, t, o, w in expected], "consumer root mismatch")
    require((size, alignment) == (48 if name == "matmul" else 24, 8) and
            container["elementVarLayout"]["binding"]["size"] == size, "consumer root extent")
    shared = []
    for v in variables:
        if v[2] != "Workgroup": continue
        pointer = native[v[1]]
        require(pointer[:2] == ["OpTypePointer", "Workgroup"], "shared pointer")
        array = native[pointer[2]]
        require(array[0] == "OpTypeArray", "shared array")
        typ = native[array[1]]
        require(typ == (["OpTypeFloat", "32"] if name == "matmul" else ["OpTypeInt", "32", "0"]),
                "shared scalar")
        shared.append(4 * constant(native, array[2]))
    require(shared == ({"reduce": [256], "matmul": [256, 256], "subgroup": []}[name]), "shared extent")
    sync = barriers(assembly)
    require(sync == ([] if name == "subgroup" else [[2, 2, 264]]*2), "barrier scope/semantics")
    builtins = sorted(re.findall(r"OpDecorate %\S+ BuiltIn (\w+)", assembly))
    require(builtins == sorted(["GlobalInvocationId", "SubgroupLocalInvocationId"] if name == "subgroup"
                              else ["LocalInvocationId", "WorkgroupId"]), "unexpected builtins")
    collectives = []
    for v in native.values():
        if v[0].startswith("OpGroupNonUniform"):
            require(len(v) == 5 and constant(native, v[2]) == 3 and v[3] == "Reduce" and
                    native[v[1]] == ["OpTypeInt", "32", "0"], "collective scope/type/operation")
            collectives.append(v[0])
    require(sorted(collectives) == (["OpGroupNonUniformIAdd"]*2 + ["OpGroupNonUniformUMin"]
                                  if name == "subgroup" else []), "unexpected collective")
    try:
        generate.inspect(reflection, assembly, name)
    except ValueError as error:
        rejection = str(error)
        require(rejection == ("unmapped or missing capability" if name == "subgroup" else
                              "shared/global resources unsupported"), "unexpected adapter rejection")
    else:
        raise ValueError("installed adapter unexpectedly accepts probe; review its profile")
    operations = Counter(re.findall(r"\b(Op\w+)\b", assembly))
    return dict(root_size=size, root_alignment=alignment, fields=list(fields), capabilities=caps,
                workgroup_bytes=shared, barriers=sync, collectives=collectives,
                physical_pointer_accesses=operations["OpPtrAccessChain"],
                function_variables=sum(v[2] == "Function" for v in variables),
                installed_adapter_rejection=rejection)


def compile_probe(name, out, compiler):
    source = HERE / f"{name}.slang"
    options = ["-target", "spirv", "-profile", "spirv_1_5", "-emit-spirv-directly",
               "-fvk-use-entrypoint-name", "-fvk-use-c-layout", "-entry", "main", "-stage", "compute"]
    if name == "subgroup":
        options += ["-capability", "spvGroupNonUniform+spvGroupNonUniformArithmetic"]
    result = subprocess.run([compiler, str(source), *options, "-reflection-json", str(out/f"{name}.json"),
                             "-o", str(out/f"{name}.spv")], text=True, capture_output=True)
    require(result.returncode == 0 and not result.stderr.strip(), "compiler diagnostic: " + result.stderr)
    generate.run("spirv-val", "--target-env", "vulkan1.4", str(out/f"{name}.spv"))
    assembly = generate.run("spirv-dis", str(out/f"{name}.spv"))
    (out/f"{name}.spvasm").write_text(assembly)
    reflected = json.loads((out/f"{name}.json").read_text())
    return reflected, assembly


def accepted_probes(out, compiler):
    """Reproduce accepted headers; compare value versus direct pointed-to access."""
    report = {}
    original = (generate.HERE/"structured.slang").read_text()
    direct = original
    for old, new in (("Block block = args.blocks[args.chosen];", "Block* block = &args.blocks[args.chosen];"),
                     ("Coefficients c = block.coefficients[coefficient];", "Coefficients* c = &block.coefficients[coefficient];"),
                     ("block.", "block->"), ("c.", "c->")):
        require(old in direct, "direct-access mutation anchor missing")
        direct = direct.replace(old, new)
    for name, source in (("structured", original), ("structured-direct", direct)):
        folder = out/name
        folder.mkdir(parents=True, exist_ok=True)
        shader = generate.HERE/"structured.slang" if name == "structured" else folder/"structured.slang"
        if name != "structured": shader.write_text(source)
        reflection, assembly, binary = generate.compile_source(shader, folder, compiler)
        header = generate.header(reflection, assembly, binary, source.encode(), "structured")
        if name == "structured":
            require(header == (generate.HERE/"structured.generated.h").read_text(), "stale accepted structured header")
        (folder/"structured.generated.h").write_text(header)
        (folder/"transform.spvasm").write_text(assembly)
        require(not re.search(r"\b(OpCopyMemory\w*|OpControlBarrier|OpMemoryBarrier|OpFunctionCall)\b", assembly),
                "unexpected structured copy/synchronization/call")
        native = definitions(assembly)
        local_arrays = []
        for v in native.values():
            if v[:1] != ["OpVariable"] or v[2] != "Function": continue
            typ = native[native[v[1]][2]]
            if typ[0] == "OpTypeArray": local_arrays.append(constant(native, typ[2]))
        require(local_arrays == ([2] if name == "structured" else []), "structured local materialization changed")
        fields, size, alignment, local, caps = generate.inspect(reflection, assembly, "structured")
        report[name] = dict(root_size=size, root_alignment=alignment, requirements=caps,
                            function_array_elements=local_arrays,
                            physical_pointer_accesses=assembly.count("OpPtrAccessChain"))
    pair = link_graphics.header(generate.HERE/"stage_vertex.slang", generate.HERE/"stage_fragment.slang",
                                out/"graphics", compiler, "pattern")
    require(pair == (generate.HERE/"pattern.generated.h").read_text(), "stale accepted graphics header")
    for stage in ("vertex", "fragment"):
        folder = out/"graphics"/stage
        assembly = generate.run("spirv-dis", str(folder/"transform.spv"))
        (folder/"transform.spvasm").write_text(assembly)
        reflection = json.loads((folder/"reflection.json").read_text())
        fields, size, alignment, local, caps = generate.inspect(reflection, assembly, "stage_"+stage)
        require(not re.search(r"\b(OpCopyMemory\w*|OpControlBarrier|OpMemoryBarrier|OpFunctionCall)\b", assembly),
                "unexpected stage copy/synchronization/call")
        report[stage] = dict(root_size=size, requirements=caps, interface=fields.io)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gpu", action="store_true")
    parser.add_argument("--timing", action="store_true", help="targeted structured source comparison; requires --gpu")
    parser.add_argument("--build-dir", type=Path, default=ROOT/"target/compiler-language")
    args = parser.parse_args()
    require(not args.timing or args.gpu, "--timing requires --gpu")
    out = args.build_dir.resolve()
    out.mkdir(parents=True, exist_ok=True)
    compiler = os.getenv("SLANGC", "slangc")
    version = subprocess.run([compiler, "-version"], text=True, capture_output=True, check=True)
    require((version.stdout+version.stderr).strip() == generate.VERSION, "compiler version mismatch")
    report = accepted_probes(out, compiler)
    for name in ("reduce", "matmul", "subgroup"):
        reflection, assembly = compile_probe(name, out, compiler)
        report[name] = inspect(name, reflection, assembly)
        if name != "subgroup":
            control = ROOT/"examples/shaders"/("reduce.spv" if name == "reduce" else "matmul-tiled.comp.spv")
            generate.run("spirv-val", "--target-env", "vulkan1.4", str(control))
            require(barriers(generate.run("spirv-dis", str(control))) == report[name]["barriers"],
                    "GLSL control barrier mismatch")
    (out/"report.json").write_text(json.dumps(report, indent=2)+"\n")
    print(json.dumps(report, indent=2), flush=True)
    if args.gpu:
        require(not os.getenv("CARGO_TARGET_DIR"), "leave CARGO_TARGET_DIR unset")
        subprocess.run(["cargo", "build", "--release", "--offline", "--locked", "-p", "ogpu"], cwd=ROOT, check=True)
        common = [*shlex.split(os.getenv("CC", "cc")), "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-I"+str(ROOT/"include")]
        for name, src in (("reduction", ROOT/"examples/reduction.c"), ("matmul", ROOT/"examples/matmul.c"),
                          ("reduction-guarded", HERE/"reduction_guarded.c")):
            subprocess.run([*common, str(src), "-L"+str(ROOT/"target/release"),
                            "-Wl,-rpath,"+str(ROOT/"target/release"), "-logpu", "-lm", "-o", str(out/name)], check=True)
        subprocess.run([*common, "-I"+str(ROOT/"vendor/Vulkan-Headers/include"),
                        "-I"+str(ROOT/"examples/learned_image/generated"), str(HERE/"subgroup_native.c"),
                        "-ldl", "-o", str(out/"subgroup-native")], check=True)
        commands = [[str(out/"reduction"), str(out/"reduce.spv")],
                    [str(out/"reduction-guarded"), str(out/"reduce.spv")],
                    [str(out/"matmul"), str(ROOT/"examples/shaders/matmul-naive.comp.spv"), str(out/"matmul.spv")],
                    [str(out/"subgroup-native"), str(out/"subgroup.spv")]]
        for name in ("structured", "structured-direct"):
            executable = out/(name+"-consumer")
            subprocess.run([*common, "-I"+str(out/name), str(generate.HERE/"structured.c"),
                            "-L"+str(ROOT/"target/release"), "-Wl,-rpath,"+str(ROOT/"target/release"),
                            "-logpu", "-o", str(executable)], check=True)
            commands.append([str(executable)])
        if args.timing:
            executable = out/"structured-timing"
            subprocess.run([*common, "-I"+str(out/"structured"), str(HERE/"structured_timing.c"),
                            "-L"+str(ROOT/"target/release"), "-Wl,-rpath,"+str(ROOT/"target/release"),
                            "-logpu", "-o", str(executable)], check=True)
            commands.append([str(executable), str(out/"structured/transform.spv"),
                             str(out/"structured-direct/transform.spv")])
        for command in commands:
            result = subprocess.run(command, text=True, capture_output=True)
            print(result.stdout, end="", flush=True)
            print(result.stderr, end="", file=sys.stderr, flush=True)
            require(result.returncode == 0 and "Validation Error:" not in result.stdout+result.stderr,
                    "GPU probe failed or validation error")
    print("Language probe PASS (compiler/native-artifact diagnostic, not installed profile support)")


if __name__ == "__main__":
    main()
