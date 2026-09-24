"""Independent corruption checks against pinned original/reordered artifacts."""
import copy
import json
import os
import re
import sys
import unittest
sys.dont_write_bytecode = True
import generate as g


class StructuredTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.build = g.ROOT / "target/compiler-structured"
        cls.reflection = json.loads((cls.build / "interface.json").read_text())
        cls.assembly = g.run("spirv-dis", str(cls.build / "transform.spv"))
        cls.binary = (cls.build / "transform.spv").read_bytes()

    def reject(self, mutate=None, assembly=None):
        reflected = copy.deepcopy(self.reflection)
        if mutate: mutate(reflected)
        with self.assertRaises((ValueError, KeyError)):
            g.inspect(reflected, self.assembly if assembly is None else assembly, "structured")

    def test_original_and_reordered(self):
        for folder, expected, local in ((self.build, [0,8,24,28],64),
                                        (self.build / "reordered", [0,16,24,28],32)):
            reflected = json.loads((folder / "interface.json").read_text())
            fields,size,alignment,group,requirements = g.inspect(reflected,g.run("spirv-dis",str(folder / "transform.spv")),"structured")
            self.assertEqual([f[2] for f in fields], expected)
            self.assertEqual((size,alignment,group),(32,8,[local,1,1]))
            self.assertEqual(requirements,["buffer_device_address","compute_queue"])
            self.assertEqual(g.uniform_size(reflected["ogpuPointeeLayouts"]["Block"]),(48,8))

    def test_no_query_parameters_in_artifact(self):
        self.assertNotIn("ogpuLayoutQuery", self.assembly)
        self.assertEqual(len(self.reflection["parameters"]),1)
        query = json.loads((self.build / "layout-query.json").read_text())
        self.assertEqual([p["name"] for p in query["parameters"]],["args","ogpuLayoutQuery_Block"])
        self.assertEqual(query["parameters"][0],self.reflection["parameters"][0])

    def test_wide_vectors(self):
        for width in (3,4):
            folder = self.build / f"vector{width}"
            reflected = json.loads((folder / "interface.json").read_text())
            fields,size,alignment,_,_ = g.inspect(reflected,g.run("spirv-dis",str(folder / "transform.spv")),"structured")
            self.assertEqual((size,alignment),(48,8))
            self.assertEqual([f[2] for f in fields],[0,32,40,44])
            self.assertTrue(any(f"structured_vector{width}_float[{width}]" in line for line in fields.declarations))

    def test_host_declarations(self):
        header = g.header(self.reflection,self.assembly,self.binary,b"test","structured")
        for text in ("sizeof(structured_type_Block) == 48", "structured_type_Coefficients",
                     "element stride", "root alignment", "uint64_t arg_blocks;"):
            self.assertIn(text,header)

    def test_missing_pointee(self):
        self.reject(lambda r:r.pop("ogpuPointeeLayouts"))

    def test_unused_pointee(self):
        self.reject(lambda r:r["ogpuPointeeLayouts"].update(Unused=copy.deepcopy(r["ogpuPointeeLayouts"]["Block"])))

    def test_pointee_name(self):
        self.reject(lambda r:r["ogpuPointeeLayouts"]["Block"].update(name="Other"))

    def test_pointee_extent(self):
        self.reject(lambda r:r["ogpuPointeeLayouts"]["Block"]["sizes"][0].update(value=40))

    def test_pointer_stride(self):
        changed = self.assembly.replace("ArrayStride 48", "ArrayStride 40")
        self.assertNotEqual(changed,self.assembly); self.reject(assembly=changed)

    def test_array_stride(self):
        changed = self.assembly.replace("ArrayStride 16", "ArrayStride 12")
        self.assertNotEqual(changed,self.assembly); self.reject(assembly=changed)

    def test_array_count_and_layout(self):
        for key,value in (("elementCount",0),("elementCount",3),("elementCount",4097),("uniformStride",20)):
            self.reject(lambda r:r["ogpuPointeeLayouts"]["Block"]["fields"][1]["type"].update({key:value}))

    def test_nested_vector_width(self):
        self.reject(lambda r:r["ogpuPointeeLayouts"]["Block"]["fields"][1]["type"]["elementType"]["fields"][0]["type"].update(elementCount=4))

    def test_nested_scalar_type(self):
        self.reject(lambda r:r["ogpuPointeeLayouts"]["Block"]["fields"][1]["type"]["elementType"]["fields"][1]["type"]["elementType"].update(scalarType="float32"))

    def test_missing_field_layout(self):
        self.reject(lambda r:r["ogpuPointeeLayouts"]["Block"]["fields"][2]["type"].pop("sizes"))

    def test_member_offsets_and_duplicates(self):
        for changed in (self.assembly.replace("%Block_c 2 Offset 40","%Block_c 2 Offset 36"),
                        self.assembly.replace("%Block_c 3 Offset 44","%Block_c 2 Offset 40"),
                        self.assembly.replace("%_Array_c_Coefficients2 0 Offset 0","%_Array_c_Coefficients2 0 Offset 4")):
            self.assertNotEqual(changed,self.assembly); self.reject(assembly=changed)

    def test_recursive_pointee(self):
        changed = re.sub(r"(%Block_c = OpTypeStruct) %\S+",r"\1 %_ptr_PhysicalStorageBuffer_Block_c",self.assembly)
        self.assertNotEqual(changed,self.assembly)
        self.reject(lambda r:r["ogpuPointeeLayouts"]["Block"]["fields"][0]["type"].update(valueType="Block"),changed)

    def test_named_pointees(self):
        self.assertEqual(g.named_pointees(self.reflection),{"Block"})
        for name in ("float4", "Foo::Bar", "Block; evil", "A<B>"):
            if name == "float4": continue  # Valid identifier, but queries require a struct.
            with self.assertRaises(ValueError): g.layouts.identifier(name)

    def test_real_divergent_push_index_rejected(self):
        source = (g.HERE / "structured.slang").read_text()
        changed = source.replace("id.x % 2 == 0 ? args.controls.mapping[0] : args.controls.mapping[1]",
                                 "args.controls.mapping[id.x % 2]")
        self.assertNotEqual(changed, source)
        folder = self.build / "rejected-divergent"; folder.mkdir(exist_ok=True)
        path = folder / "structured.slang"; path.write_text(changed)
        reflection, assembly, _ = g.compile_source(path, folder, os.getenv("SLANGC", "slangc"))
        # spirv-val passed, but that does not establish dynamic uniformity.
        with self.assertRaisesRegex(ValueError, "push-array index uniformity unproven"):
            g.inspect(reflection, assembly, "structured")

    def test_c_layout_alone_does_not_enable_scalar_block_layout(self):
        text = (g.HERE / "structured.slang").read_text().replace("float2 gainBias", "float3 gainBias")
        folder = self.build / "rejected-native-alignment"; folder.mkdir(exist_ok=True)
        path = folder / "structured.slang"; path.write_text(text)
        with self.assertRaisesRegex(RuntimeError, "spirv-val failed"):
            g.compile_source(path, folder, os.getenv("SLANGC", "slangc"))

    def test_uniformity_proof(self):
        defs = {
            "%uint": ["OpTypeInt", "32", "0"],
            "%ptr": ["OpTypePointer", "PushConstant", "%uint"],
            "%root": ["OpVariable", "%ptr", "PushConstant"],
            "%zero": ["OpConstant", "%uint", "0"],
            "%field": ["OpAccessChain", "%ptr", "%root", "%zero"],
            "%index": ["OpLoad", "%uint", "%field"],
            "%sum": ["OpIAdd", "%uint", "%index", "%zero"],
            "%element": ["OpAccessChain", "%ptr", "%root", "%sum"],
        }
        g.check_push_indices(defs) # Runtime value from uniform root, not just a literal.
        for bad in (["OpPhi", "%uint", "%zero", "%label"],
                    ["OpFunctionCall", "%uint", "%function"],
                    ["OpLoad", "%uint", "%unknown"],
                    ["OpIAdd", "%uint", "%sum", "%zero"]):
            broken = copy.deepcopy(defs); broken["%sum"] = bad
            with self.assertRaisesRegex(ValueError, "uniformity unproven"): g.check_push_indices(broken)


if __name__ == "__main__": unittest.main()
