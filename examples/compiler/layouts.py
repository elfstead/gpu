"""Verified C-layout subset, including reflection-only pointee metadata.

No shader execution, host packing or runtime reflection. Native IDs are checked
independently even when Slang emits multiple structurally identical type IDs.
"""
import re


def require(okay, message):
    if not okay:
        raise ValueError(message)


def size_align(typ):
    sizes = typ["sizes"]
    require(len(sizes) == 1 and sizes[0]["kind"] == "uniform", "non-uniform layout")
    size, alignment = sizes[0]["value"], sizes[0]["alignment"]
    require(type(size) is int and 0 < size <= 1024*1024 and alignment in (4, 8), "unsupported layout size/alignment")
    return size, alignment


def identifier(name):
    require(isinstance(name, str) and re.fullmatch(r"[A-Za-z][A-Za-z0-9_]*", name), "unsupported layout identifier")
    return name


class Fields(list):
    def __init__(self, fields, declarations, floating):
        super().__init__(fields)
        self.declarations = declarations
        self.floating = floating


def struct_lines(name, fields, size, alignment, root=False):
    lines = [f"typedef struct {name} {{"]
    cursor = 0
    for index, (field, typ, offset, width) in enumerate(fields):
        if offset > cursor: lines.append(f"    uint8_t padding_{index}[{offset-cursor}];")
        lines.append(f"    {typ} {field};")
        cursor = offset + width
    if size > cursor: lines.append(f"    uint8_t padding_tail[{size-cursor}];")
    label = "root" if root else name
    lines += [f"}} {name};", f'_Static_assert(sizeof({name}) == {size}, "{label} size");',
              f'_Static_assert(_Alignof({name}) == {alignment}, "{label} alignment");']
    for field, _, offset, _ in fields:
        lines.append(f'_Static_assert(offsetof({name}, {field}) == {offset}, "{field} offset");')
    return lines


class Inspector:
    def __init__(self, definitions, assembly, pointees, physical, name):
        self.native = definitions
        self.assembly = assembly
        self.pointees = pointees
        self.physical = physical
        self.prefix = name
        self.active = set()
        self.types = {}
        self.declarations = []
        self.floating = False
        self.used_pointees = set()

    def offsets(self, native_id, count):
        rows = re.findall(rf"^\s*OpMemberDecorate {re.escape(native_id)} (\d+) (.+)$", self.assembly, re.M)
        require(len(rows) == count, "unexpected member decoration")
        require(all(re.fullmatch(r"Offset \d+", value) for _, value in rows), "unsupported member decoration")
        require(sorted(int(i) for i, _ in rows) == list(range(count)), "duplicate/missing member decoration")
        return {int(i): int(value.split()[1]) for i, value in rows}

    def stride(self, native_id, expected):
        strides = re.findall(rf"^\s*OpDecorate {re.escape(native_id)} ArrayStride (\d+)$", self.assembly, re.M)
        require(strides == [str(expected)], "array/pointer stride mismatch")

    def declare(self, name, signature, lines):
        if name in self.types:
            require(self.types[name] == signature, "conflicting generated type layouts")
        else:
            self.types[name] = signature
            self.declarations.extend(lines)

    def inspect(self, reflected, native_id, root=False):
        require(native_id not in self.active and len(self.active) < 32, "recursive/deep argument layout unsupported")
        self.active.add(native_id)
        try:
            return self.visit(reflected, native_id, root)
        finally:
            self.active.remove(native_id)

    def visit(self, reflected, native_id, root):
        native = self.native[native_id]
        kind = reflected["kind"]
        if kind == "scalar":
            scalars = {"uint32": ("uint32_t", ["OpTypeInt", "32", "0"]),
                       "float32": ("float", ["OpTypeFloat", "32"])}
            require(reflected.get("scalarType") in scalars, "unsupported scalar")
            c_type, expected = scalars[reflected["scalarType"]]
            require(native == expected, "scalar type mismatch")
            self.floating |= c_type == "float"
            size, alignment = 4, 4
        elif kind == "pointer":
            require(self.physical and native[:2] == ["OpTypePointer", "PhysicalStorageBuffer"], "pointer type mismatch")
            pointee_name = reflected["valueType"]
            scalar = {"uint": "uint32", "float": "float32"}.get(pointee_name)
            if scalar:
                target = dict(kind="scalar", scalarType=scalar)
            else:
                identifier(pointee_name)
                require(pointee_name in self.pointees, "missing reflected pointee layout")
                target = self.pointees[pointee_name]
                require(target["kind"] == "struct" and target["name"] == pointee_name, "pointee name mismatch")
                self.used_pointees.add(pointee_name)
            _, stride, _, _ = self.inspect(target, native[2])
            self.stride(native_id, stride)
            c_type, size, alignment = "uint64_t", 8, 8
        elif kind in ("vector", "array"):
            count = reflected["elementCount"]
            require(type(count) is int and 0 < count <= 4096, "unsupported element count")
            if kind == "vector":
                require(count in (2, 3, 4) and reflected["elementType"]["kind"] == "scalar", "unsupported vector")
                require(native[0] == "OpTypeVector" and native[2] == str(count), "vector type/count mismatch")
                element_id = native[1]
            else:
                # Pinned C-layout Slang wraps fixed arrays in a single data member.
                if native[0] == "OpTypeStruct":
                    require(len(native) == 2 and self.offsets(native_id, 1) == {0: 0}, "invalid array wrapper")
                    names = re.findall(rf'^\s*OpMemberName {re.escape(native_id)} (\d+) "([^"]+)"$', self.assembly, re.M)
                    require(names == [("0", "data")], "invalid array wrapper name")
                    native_id = native[1]; native = self.native[native_id]
                require(native[0] == "OpTypeArray" and len(native) == 3, "array type mismatch")
                constant = self.native[native[2]]
                require(constant[0] == "OpConstant" and len(constant) == 3 and constant[2] == str(count)
                        and self.native[constant[1]] in (["OpTypeInt", "32", "0"], ["OpTypeInt", "32", "1"]),
                        "array count mismatch")
                element_id = native[1]
            element, width, alignment, _ = self.inspect(reflected["elementType"], element_id)
            size = width * count
            if kind == "array":
                require(reflected["uniformStride"] == width, "reflected array stride mismatch")
                self.stride(native_id, width)
            c_type = f"{self.prefix}_{kind}{count}_{element}"
            self.declare(c_type, (kind, count, element, size, alignment),
                         [f"typedef {element} {c_type}[{count}];",
                          f'_Static_assert(sizeof({c_type}) == {size}, "array/vector size");',
                          f'_Static_assert(_Alignof({c_type}) == {alignment}, "array/vector alignment");',
                          f'_Static_assert(sizeof((({c_type}*)0)[0][0]) == {width}, "element stride");'])
        elif kind == "struct":
            fields = reflected["fields"]
            require(native[0] == "OpTypeStruct" and len(native)-1 == len(fields) and fields, "struct field count mismatch")
            offsets = self.offsets(native_id, len(fields))
            names = re.findall(rf'^\s*OpMemberName {re.escape(native_id)} (\d+) "([^"]+)"$', self.assembly, re.M)
            require(len(names) == len(fields) and {int(i): n for i, n in names} ==
                    {i: f["name"] for i, f in enumerate(fields)}, "struct field name mismatch")
            members, cursor, alignment = [], 0, 1
            for i, (field, member_id) in enumerate(zip(fields, native[1:])):
                name = "arg_" + identifier(field["name"])
                require(name not in [f[0] for f in members], "duplicate field name")
                c_type, width, align, _ = self.inspect(field["type"], member_id)
                require(size_align(field["type"]) == (width, align), "missing/inconsistent field layout")
                binding = field["binding"]
                offset = binding["offset"]
                stride = field["type"]["uniformStride"] if field["type"]["kind"] == "array" else 0
                if field["type"]["kind"] == "vector": stride = 4
                require(binding == dict(kind="uniform", offset=offset, size=width, elementStride=stride)
                        and offsets[i] == offset, "field offset/size/stride mismatch")
                require(offset == (cursor + align - 1)//align*align, "unexpected C field layout")
                members.append((name, c_type, offset, width))
                cursor = offset + width; alignment = max(alignment, align)
            size = (cursor + alignment - 1)//alignment*alignment
            c_type = f"{self.prefix}_type_{identifier(reflected['name'])}"
            if not root:
                self.declare(c_type, (members, size, alignment), struct_lines(c_type, members, size, alignment))
            require(size_align(reflected) == (size, alignment), "struct size/alignment mismatch")
            return c_type, size, alignment, members
        else:
            raise ValueError("unsupported argument layout kind: " + str(kind))
        if "sizes" in reflected:
            require(size_align(reflected) == (size, alignment), "type size/alignment mismatch")
        else:
            # Slang omits layout on vector scalar elements. Only those may omit it.
            require(kind == "scalar", "missing type layout")
        require(size <= 1024*1024, "argument aggregate too large")
        return c_type, size, alignment, None


def inspect_root(root, native_id, definitions, assembly, pointees, physical, name):
    inspector = Inspector(definitions, assembly, pointees, physical, name)
    _, size, alignment, fields = inspector.inspect(root, native_id, root=True)
    require(inspector.used_pointees == set(pointees), "unused/unverified pointee metadata")
    # Keep legacy scalar-pointer-only headers byte-identical (no float host fields).
    floating = any(typ == "float" for _, typ, _, _ in fields) or bool(inspector.declarations and inspector.floating)
    return Fields(fields, inspector.declarations, floating), size, alignment
