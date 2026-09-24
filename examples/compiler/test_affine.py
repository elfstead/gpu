"""FP32 root cross-checks against actual pinned Slang artifacts."""
import copy
import json
import sys
import unittest
sys.dont_write_bytecode = True
import generate as g


class AffineTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.build = g.ROOT / "target/compiler-affine"
        cls.reflection = json.loads((cls.build / "reflection.json").read_text())
        cls.binary = (cls.build / "transform.spv").read_bytes()
        cls.assembly = g.run("spirv-dis", str(cls.build / "transform.spv"))

    def test_original(self):
        fields, size, alignment, local, requirements = g.inspect(self.reflection, self.assembly)
        self.assertEqual(fields, [("arg_data","uint64_t",0,8),("arg_count","uint32_t",8,4),
                                  ("arg_scale","float",12,4),("arg_bias","float",16,4)])
        self.assertEqual((size, alignment, local), (24,8,[64,1,1]))
        self.assertEqual(requirements, ["buffer_device_address","compute_queue"])

    def test_reordered(self):
        folder = self.build / "reordered"
        fields, size, alignment, local, _ = g.inspect(json.loads((folder / "reflection.json").read_text()),
                                                     g.run("spirv-dis", str(folder / "transform.spv")))
        self.assertEqual([(f[0],f[2]) for f in fields],
                         [("arg_bias",0),("arg_scale",4),("arg_count",8),("arg_data",16)])
        self.assertEqual((size, alignment, local), (24,8,[32,1,1]))

    def test_host_assertions(self):
        header = g.header(self.reflection,self.assembly,self.binary,b"fixture","affine")
        for text in ("FLT_MANT_DIG == 24", "sizeof(float) == 4", "float arg_scale;",
                     "uint8_t padding_tail[4]", "offsetof(AffineArguments, arg_bias) == 16"):
            self.assertIn(text, header)

    def test_native_scalar_disagreement(self):
        for changed in (self.assembly.replace("OpTypeFloat 32", "OpTypeFloat 16"),
                        self.assembly.replace("2 Offset 12", "2 Offset 8")):
            with self.assertRaises(ValueError): g.inspect(self.reflection, changed)

    def test_reflected_scalar_disagreement(self):
        for scalar in ("uint32", "float16", "float64", "bool"):
            reflection = copy.deepcopy(self.reflection)
            container = reflection["parameters"][0]["type"]
            container["elementType"]["fields"][2]["type"]["scalarType"] = scalar
            container["elementVarLayout"]["type"] = copy.deepcopy(container["elementType"])
            with self.assertRaises(ValueError): g.inspect(reflection, self.assembly)

    def test_layout_disagreement(self):
        for key, value in (("value", 8), ("alignment", 8)):
            reflection = copy.deepcopy(self.reflection)
            container = reflection["parameters"][0]["type"]
            container["elementType"]["fields"][2]["type"]["sizes"][0][key] = value
            container["elementVarLayout"]["type"] = copy.deepcopy(container["elementType"])
            with self.assertRaises(ValueError): g.inspect(reflection, self.assembly)


if __name__ == "__main__": unittest.main()
