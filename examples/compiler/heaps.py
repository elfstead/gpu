"""Bounded native heap interface checks; no ownership or index-range inference."""
import re

# SPIR-V storage formats with 32-bit shader scalars. These names describe shader
# formats, not a promise that every device supports a corresponding view format.
STORAGE_FORMATS = {
    **dict.fromkeys(("Rgba32f", "Rgba16f", "R32f", "Rgba8", "Rgba8Snorm", "Rg32f",
                     "Rg16f", "R11fG11fB10f", "R16f", "Rgba16", "Rgb10A2", "Rg16",
                     "Rg8", "R16", "R8", "Rgba16Snorm", "Rg16Snorm", "Rg8Snorm",
                     "R16Snorm", "R8Snorm"), "float"),
    **dict.fromkeys(("Rgba32i", "Rgba16i", "Rgba8i", "R32i", "Rg32i", "Rg16i", "Rg8i",
                     "R16i", "R8i"), "sint"),
    **dict.fromkeys(("Rgba32ui", "Rgba16ui", "Rgba8ui", "R32ui", "Rgb10a2ui", "Rg32ui",
                     "Rg16ui", "Rg8ui", "R16ui", "R8ui"), "uint"),
}


def storage_requirements(definitions, assembly, resources):
    """Per-kind union of format flags, not descriptor-index/lifetime inference.

    Storage descriptors may only flow directly into reads/writes in this
    adapter profile. No guessed dataflow through calls, phi nodes or texel pointers.
    Capabilities alone overstate requirements in pinned Slang (it emits both).
    """
    result = {kind: {"STORAGE"} for kind in resources.values() if kind.startswith("storage_image_")}
    loaded = {identity: resources[value[1]] for identity, value in definitions.items()
              if value[0] == "OpLoad" and value[1] in resources
              and resources[value[1]].startswith("storage_image_")}
    for line in assembly.splitlines():
        tokens = line.split()
        if not tokens: continue
        instruction = tokens[2:] if len(tokens) > 2 and tokens[1] == "=" else tokens
        for identity in loaded.keys() & set(instruction[1:]):
            kind = loaded[identity]
            if instruction[0] == "OpImageRead" and len(instruction) >= 4 and instruction[2] == identity:
                if "_unknown_" in kind: result[kind].add("STORAGE_READ_WITHOUT_FORMAT")
            elif instruction[0] == "OpImageWrite" and len(instruction) >= 4 and instruction[1] == identity:
                if "_unknown_" in kind: result[kind].add("STORAGE_WRITE_WITHOUT_FORMAT")
            else:
                require(instruction[0] in ("OpName", "OpDecorate"), "unsupported storage descriptor flow")
    return {kind: sorted(flags) for kind, flags in sorted(result.items())}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def inspect(definitions, assembly, enabled):
    heaps = {}
    for identity, value in definitions.items():
        if value[0] != "OpUntypedVariableKHR":
            continue
        require(enabled and len(value) == 3 and value[2] == "UniformConstant",
                "unsupported untyped variable")
        require(definitions.get(value[1]) == ["OpTypeUntypedPointerKHR", "UniformConstant"],
                "unsupported heap pointer")
        decorations = re.findall(rf"^\s*OpDecorate {re.escape(identity)} (.+)$", assembly, re.M)
        require(len(decorations) == 1 and decorations[0] in
                ("BuiltIn ResourceHeapEXT", "BuiltIn SamplerHeapEXT"), "unsupported heap builtin")
        kind = decorations[0].split()[1]
        require(kind not in heaps.values(), "duplicate heap builtin")
        heaps[identity] = kind
    require(bool(heaps) == enabled, "heap capability/declaration mismatch")
    for identity in heaps:
        uses = [v for v in definitions.values() if identity in v[1:]]
        require(uses and all(len(v) == 5 and v[0] == "OpUntypedAccessChainKHR" and v[3] == identity
                            for v in uses), "unsupported heap base use")
    resources, arrays, sizes, used_heaps = {}, set(), set(), set()
    for identity, value in definitions.items():
        if value[0] == "OpTypeSampler":
            require(value == ["OpTypeSampler"], "unsupported sampler type")
            resources[identity] = "sampler"
        elif value[0] == "OpTypeImage":
            require(len(value) == 8 and value[2:6] == ["2D", "2", "0", "0"],
                    "unsupported heap image type")
            scalar = {("OpTypeFloat", "32"): "float", ("OpTypeInt", "32", "1"): "sint",
                      ("OpTypeInt", "32", "0"): "uint"}.get(tuple(definitions.get(value[1], [])))
            require(scalar is not None, "unsupported heap image scalar")
            use, fmt = value[6:]
            if use == "1" and fmt == "Unknown":
                kind = "sampled_image_2d_" + scalar
            else:
                require(use == "2" and (fmt == "Unknown" or STORAGE_FORMATS.get(fmt) == scalar),
                        "unsupported heap image format/use")
                kind = "storage_image_2d_" + ("unknown_" + scalar if fmt == "Unknown" else fmt.lower())
            resources[identity] = kind
    used_resources = set()
    for identity, value in definitions.items():
        if value[0] != "OpUntypedAccessChainKHR":
            continue
        require(enabled and len(value) == 5 and value[3] in heaps,
                "unsupported heap access path")
        require(definitions.get(value[1]) == ["OpTypeUntypedPointerKHR", "UniformConstant"],
                "unsupported heap access pointer")
        array = definitions.get(value[2], [])
        require(len(array) == 2 and array[0] == "OpTypeRuntimeArray" and array[1] in resources,
                "unsupported heap descriptor array")
        resource = array[1]
        expected = "SamplerHeapEXT" if resources[resource] == "sampler" else "ResourceHeapEXT"
        require(heaps[value[3]] == expected, "descriptor kind/heap mismatch")
        strides = re.findall(rf"^\s*OpDecorateId {re.escape(value[2])} ArrayStrideIdEXT (%\S+)$", assembly, re.M)
        require(len(strides) == 1, "missing/duplicate heap descriptor stride")
        size = definitions.get(strides[0], [])
        require(len(size) == 3 and size[0] == "OpConstantSizeOfEXT" and size[2] == resource
                and definitions.get(size[1]) == ["OpTypeInt", "32", "0"],
                "descriptor stride must be its native size")
        arrays.add(value[2]); sizes.add(strides[0])
        used_resources.add(resource); used_heaps.add(value[3])
        # This subset loads descriptors directly. No descriptor address escapes,
        # nested untyped arithmetic, texel pointers or buffer descriptors.
        uses = [v for v in definitions.values() if identity in v[1:]]
        require(uses and all(v == ["OpLoad", resource, identity] for v in uses),
                "unsupported descriptor pointer use")
    require(used_resources == resources.keys() and used_heaps == heaps.keys(), "unused/unmapped heap resource")
    for identity, value in definitions.items():
        op = value[0]
        if op == "OpTypeRuntimeArray":
            require(identity in arrays, "unmapped runtime descriptor array")
        if op == "OpConstantSizeOfEXT":
            require(identity in sizes, "unmapped descriptor size")
        if op == "OpTypeUntypedPointerKHR":
            require(enabled and value == [op, "UniformConstant"], "unsupported untyped pointer type")
        if "Untyped" in op:
            require(op in ("OpTypeUntypedPointerKHR", "OpUntypedVariableKHR", "OpUntypedAccessChainKHR"),
                    "unsupported untyped operation")
        require(op not in ("OpTypeBufferEXT", "OpBufferPointerEXT"), "buffer descriptors unsupported")
    # Reject unexamined stride decorations, including a conflicting literal stride.
    decorated = re.findall(r"^\s*OpDecorateId (.+)$", assembly, re.M)
    require(len(decorated) == len(arrays) and all(
        len(d.split()) == 3 and d.split()[0] in arrays and d.split()[1] == "ArrayStrideIdEXT"
        for d in decorated), "unmapped descriptor decoration")
    for array in arrays:
        require(not re.search(rf"^\s*OpDecorate {re.escape(array)} ", assembly, re.M),
                "conflicting descriptor array decoration")
    # Non-result instructions are absent from definitions (e.g. OpStore and
    # OpReturnValue). They must not let the checked heap/descriptor pointers escape.
    restricted = set(heaps) | {key for key, value in definitions.items() if value[0] == "OpUntypedAccessChainKHR"}
    in_function = False
    for line in assembly.splitlines():
        tokens = line.split()
        if "OpFunction" in tokens: in_function = True
        if in_function and "=" not in tokens:
            require(not restricted.intersection(tokens), "heap pointer escapes through instruction")
        if "OpFunctionEnd" in tokens: in_function = False
    return heaps, sorted(set(resources.values())), storage_requirements(definitions, assembly, resources)
