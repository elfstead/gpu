"""Generator rejection tests using the real pinned compiler output."""
import copy
import json
import unittest
import sys
sys.dont_write_bytecode = True
import generate


class GeneratorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        directory = generate.ROOT / "target/compiler-workflow"
        cls.reflection = json.loads((directory / "reflection.json").read_text())
        cls.assembly = generate.run("spirv-dis", str(directory / "transform.spv"))

    def reject(self, mutate=None, assembly=None):
        reflected = copy.deepcopy(self.reflection)
        if mutate:
            mutate(reflected)
        with self.assertRaises((ValueError, KeyError)):
            generate.inspect(reflected, self.assembly if assembly is None else assembly)

    def test_real_layout(self):
        fields, size, align, local, required = generate.inspect(self.reflection, self.assembly)
        self.assertEqual([(f[0], f[2]) for f in fields], [("arg_data", 0), ("arg_count", 8)])
        self.assertEqual((size, align, local), (16, 8, [64, 1, 1]))
        self.assertEqual(required, ["buffer_device_address", "compute_queue"])

    def test_offset_mismatch(self):
        self.reject(assembly=self.assembly.replace("1 Offset 8", "1 Offset 4"))

    def test_type_mismatch(self):
        self.reject(assembly=self.assembly.replace("OpTypeInt 32 0", "OpTypeInt 16 0"))

    def test_entry_mismatch(self):
        self.reject(assembly=self.assembly.replace('"main"', '"other"'))

    def test_local_mismatch(self):
        self.reject(assembly=self.assembly.replace("LocalSize 64 1 1", "LocalSize 32 1 1"))

    def test_unknown_capability(self):
        self.reject(assembly=self.assembly + "\nOpCapability Int64\n")

    def test_unknown_extension(self):
        self.reject(assembly=self.assembly + '\nOpExtension "SPV_unknown"\n')

    def test_optional_capability_is_explicit(self):
        *_, required = generate.inspect(self.reflection, self.assembly + "\nOpCapability Float16\n")
        self.assertIn("shader_float16", required)

    def test_unexpected_builtin(self):
        self.reject(assembly=self.assembly.replace("BuiltIn GlobalInvocationId", "BuiltIn SubgroupSize"))

    def test_unexpected_execution_mode(self):
        self.reject(assembly=self.assembly + "\nOpExecutionMode %main SubgroupSize 32\n")

    def test_shared_storage(self):
        self.reject(assembly=self.assembly + "\n%extra = OpVariable %uint Workgroup\n")

    def test_specialization(self):
        self.reject(assembly=self.assembly + "\n%extra = OpSpecConstant %uint 32\n")

    def test_descriptor_binding(self):
        self.reject(assembly=self.assembly + "\nOpDecorate %args DescriptorSet 0\n")

    def test_extra_root(self):
        self.reject(lambda r: r["parameters"].append(copy.deepcopy(r["parameters"][0])))

    def test_reflection_disagreement(self):
        self.reject(lambda r: r["parameters"][0]["type"]["elementType"]["sizes"][0].update(value=24))

    def test_unsupported_root_field(self):
        def mutate(r):
            container = r["parameters"][0]["type"]
            container["elementType"]["fields"][0]["type"]["kind"] = "array"
            container["elementVarLayout"]["type"] = copy.deepcopy(container["elementType"])
        self.reject(mutate)


if __name__ == "__main__":
    unittest.main()
