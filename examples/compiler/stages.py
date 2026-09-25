"""Bounded reflected/native stage IO and offline vertex/fragment matching."""
import re


def require(condition, message):
    if not condition:
        raise ValueError(message)


def reflected_type(typ):
    if typ["kind"] == "scalar":
        result = (typ["scalarType"], 1)
    else:
        require(typ["kind"] == "vector" and typ["elementType"]["kind"] == "scalar",
                "unsupported entry interface type")
        result = (typ["elementType"]["scalarType"], typ["elementCount"])
    require(result[0] in ("uint32", "float32") and type(result[1]) is int and 1 <= result[1] <= 4,
            "unsupported stage scalar/vector")
    return result


def reflected(entry):
    stage = entry["stage"]
    require(len(entry["parameters"]) == 1, "unsupported entry input count")
    expected = []

    def field(value, storage):
        semantic = value.get("semanticName")
        scalar, lanes = reflected_type(value["type"])
        builtin = {
            ("compute", "Input", "SV_DISPATCHTHREADID"): ("GlobalInvocationId", "uint32", 3),
            ("vertex", "Input", "SV_VULKANVERTEXID"): ("VertexIndex", "uint32", 1),
            ("vertex", "Output", "SV_POSITION"): ("Position", "float32", 4),
            ("fragment", "Input", "SV_POSITION"): ("FragCoord", "float32", 4),
        }.get((stage, storage, semantic))
        if builtin:
            require((scalar, lanes) == builtin[1:] and "binding" not in value, "builtin reflection mismatch")
            # Slang exposes an unsigned source VertexID with signed native builtin.
            expected.append((storage, "BuiltIn "+builtin[0], "int32" if builtin[0] == "VertexIndex" else scalar, lanes))
            return
        require((stage, storage) in (("vertex", "Output"), ("fragment", "Input"), ("fragment", "Output")),
                "unsupported stage varying direction")
        require(semantic == ("SV_TARGET" if storage == "Output" and stage == "fragment" else "TEXCOORD"),
                "unsupported stage semantic")
        binding = value["binding"]
        index = binding.get("index")
        require(type(index) is int and 0 <= index < 8 and binding == {"kind": "varying"+storage, "index": index},
                "unsupported varying binding")
        if stage == "fragment" and storage == "Output":
            require(index == 0 and (scalar, lanes) == ("float32", 4), "unsupported fragment output")
        expected.append((storage, f"Location {index}", scalar, lanes))

    def value(item, storage):
        if item["type"]["kind"] != "struct":
            field(item, storage)
            return
        require((stage, storage) in (("vertex", "Output"), ("fragment", "Input")), "unsupported stage struct")
        fields = item["type"]["fields"]
        require(1 <= len(fields) <= 9, "unsupported stage field count")
        names = [f["name"] for f in fields]
        require(len(set(names)) == len(names), "duplicate stage field")
        before = len(expected)
        for member in fields: field(member, storage)
        varying = [int(row[1].split()[1]) for row in expected[before:] if row[1].startswith("Location")]
        require(varying and sorted(varying) == list(range(len(varying))), "noncontiguous reflected stage struct")
        require(item["binding"] == {"kind": "varying"+storage, "index": 0, "count": len(varying)}
                and item["type"]["sizes"] == [{"kind": "varying"+storage, "value": len(varying)}],
                "stage aggregate binding mismatch")

    value(entry["parameters"][0], "Input")
    if stage == "compute":
        require("result" not in entry, "compute output unsupported")
    else:
        value(entry["result"], "Output")
    required_builtins = {"compute": [("Input", "BuiltIn GlobalInvocationId")],
                         "vertex": [("Input", "BuiltIn VertexIndex"), ("Output", "BuiltIn Position")],
                         "fragment": [("Input", "BuiltIn FragCoord")]}[stage]
    require(sorted((row[0], row[1]) for row in expected if row[1].startswith("BuiltIn")) == sorted(required_builtins),
            "required stage builtin missing/duplicated")
    require(len({(row[0], row[1]) for row in expected}) == len(expected), "overlapping stage locations")
    return expected


def inspect(entry, definitions, variables, assembly, heap_ids):
    expected = reflected(entry)
    actual, records, interface_ids = [], [], set()
    for identity, variable in variables:
        if variable[2] not in ("Input", "Output"): continue
        storage = variable[2]
        interface_ids.add(identity)
        pointer = definitions[variable[1]]
        require(pointer[:2] == ["OpTypePointer", storage], "invalid interface pointer")
        typ, lanes = definitions[pointer[2]], 1
        if typ[0] == "OpTypeVector":
            lanes, typ = int(typ[2]), definitions[typ[1]]
        scalar = {("OpTypeFloat", "32"): "float32", ("OpTypeInt", "32", "0"): "uint32",
                  ("OpTypeInt", "32", "1"): "int32"}.get(tuple(typ))
        decorations = re.findall(rf"^\s*OpDecorate {re.escape(identity)} (.+)$", assembly, re.M)
        locations = [d for d in decorations if re.fullmatch(r"(?:Location \d+|BuiltIn \w+)", d)]
        require(len(locations) == 1 and len(set(decorations)) == len(decorations), "unsupported interface decorations")
        location = locations[0]
        interpolation = set(decorations) - {location}
        require(interpolation in (set(), {"Flat"}, {"NoPerspective"}), "unsupported interpolation decoration")
        require(not interpolation or location.startswith("Location") and
                (entry["stage"], storage) in (("vertex", "Output"), ("fragment", "Input")),
                "interpolation outside varying interface")
        if location.startswith("Location") and entry["stage"] == "fragment" and storage == "Input" and scalar != "float32":
            require(interpolation == {"Flat"}, "integer fragment input must be flat")
        actual.append((storage, location, scalar, lanes))
        if location.startswith("Location"):
            records.append(dict(direction=storage, location=int(location.split()[1]), scalar=scalar, lanes=lanes,
                                interpolation=next(iter(interpolation), "Smooth")))
    require(sorted(actual) == sorted(expected), "native entry interface mismatch")
    decorated = re.findall(r"OpDecorate (%\S+) (?:BuiltIn|Location) \w+", assembly)
    require(set(decorated) == interface_ids | heap_ids.keys()
            and len(decorated) == len(interface_ids)+len(heap_ids), "extra interface decoration")
    require(not re.search(r"OpMemberDecorate %\S+ \d+ (BuiltIn|Location)\b", assembly), "member interfaces unsupported")
    return sorted(records, key=lambda r: (r["direction"], r["location"]))


def link(vertex, fragment):
    """Checked records only. Exact scalar/vector widths, no component packing.

    Interpolation belongs to the fragment input; unlike types/locations it need
    not match vertex decorations in our monolithic raster pipeline. Extra vertex
    outputs are legal. This is a bounded offline check, not a SPIR-V linker.
    """
    outputs = {r["location"]: r for r in vertex if r["direction"] == "Output"}
    inputs = [r for r in fragment if r["direction"] == "Input"]
    for target in inputs:
        source = outputs.get(target["location"])
        require(source is not None, f"fragment location {target['location']} has no vertex producer")
        require((source["scalar"], source["lanes"]) == (target["scalar"], target["lanes"]),
                f"stage type mismatch at location {target['location']}")
    return inputs
