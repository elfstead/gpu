"""Check the real six-stage compiler output, then corrupt its claimed contract."""
import copy
import json
import re
import sys
import unittest
sys.dont_write_bytecode = True
import generate_interfaces as application
generate = application.generate


class InterfaceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.outputs = {}
        for variant in ("original", "mutated"):
            for name, _ in application.SOURCES:
                directory = application.ROOT / "target/learned-image/compiler" / variant / name
                cls.outputs[variant, name] = (json.loads((directory / "reflection.json").read_text()),
                                             generate.run("spirv-dis", str(directory / "transform.spv")))

    def reject(self, name, mutate=None, assembly=None):
        reflection, native = copy.deepcopy(self.outputs["original", name])
        if mutate:
            mutate(reflection)
        with self.assertRaises((ValueError, KeyError)):
            generate.inspect(reflection, native if assembly is None else assembly(native))

    def test_all_stages_and_mutated_interfaces(self):
        for name, stage in application.SOURCES:
            original = generate.inspect(*self.outputs["original", name])
            mutated = generate.inspect(*self.outputs["mutated", name])
            if stage == "vertex":
                self.assertEqual(original, ([], 0, 1, [], ["graphics_queue"]))
                self.assertEqual(mutated, original)
            else:
                self.assertNotEqual(original[0], mutated[0])
                self.assertEqual([f[0] for f in original[0]], list(reversed([f[0] for f in mutated[0]])))
                self.assertIn("buffer_device_address", original[-1])
                self.assertEqual(original[3], [64, 1, 1] if stage == "compute" else [])
                self.assertEqual(mutated[3], [32, 1, 1] if stage == "compute" else [])
                self.assertIn("compute_queue" if stage == "compute" else "graphics_queue", original[-1])

    def test_float_pointer_type_disagreement(self):
        def mutate(r):
            container = r["parameters"][0]["type"]
            container["elementType"]["fields"][0]["type"]["valueType"] = "uint"
            container["elementVarLayout"]["type"] = copy.deepcopy(container["elementType"])
        self.reject("hidden", mutate=mutate)

    def test_float_pointer_native_width(self):
        self.reject("hidden", assembly=lambda s: s.replace("OpTypeFloat 32", "OpTypeFloat 64"))

    def test_pointer_stride(self):
        self.reject("hidden", assembly=lambda s: s.replace("ArrayStride 4", "ArrayStride 8"))

    def test_missing_address_capability(self):
        self.reject("hidden", assembly=lambda s: s.replace("OpCapability PhysicalStorageBufferAddresses", ""))

    def test_unsupported_base_vertex_capability(self):
        self.reject("fullscreen", assembly=lambda s: s + "\nOpCapability DrawParameters\n")

    def test_vertex_semantic_disagreement(self):
        self.reject("fullscreen", mutate=lambda r: r["entryPoints"][0]["parameters"][0].update(semanticName="SV_VERTEXID"))

    def test_vertex_builtin_disagreement(self):
        self.reject("fullscreen", assembly=lambda s: s.replace("BuiltIn VertexIndex", "BuiltIn BaseVertex"))

    def test_vertex_input_width(self):
        self.reject("fullscreen", assembly=lambda s: s.replace("OpTypeInt 32 1", "OpTypeInt 64 1"))

    def test_stage_disagreement(self):
        self.reject("display", assembly=lambda s: s.replace("OpEntryPoint Fragment", "OpEntryPoint Vertex"))

    def test_fragment_location(self):
        self.reject("display", assembly=lambda s: s.replace("Location 0", "Location 1"))

    def test_fragment_output_width(self):
        self.reject("display", assembly=lambda s: s.replace("OpTypeVector %float 4", "OpTypeVector %float 3"))

    def test_fragment_reflected_output(self):
        self.reject("display", mutate=lambda r: r["entryPoints"][0]["result"]["type"].update(elementCount=3))

    def test_fragment_reflected_location(self):
        self.reject("display", mutate=lambda r: r["entryPoints"][0]["result"]["binding"].update(index=1))

    def test_graphics_local_dimensions(self):
        self.reject("display", mutate=lambda r: r["entryPoints"][0].update(threadGroupSize=[64, 1, 1]))

    def test_fragment_mode(self):
        self.reject("display", assembly=lambda s: s.replace("OriginUpperLeft", "OriginLowerLeft"))

    def test_extra_input(self):
        self.reject("display", assembly=lambda s: s + "\n%extra = OpVariable %_ptr_Input_v4float Input\n")

    def test_hidden_entry_variable(self):
        self.reject("display", assembly=lambda s: re.sub(r'(OpEntryPoint Fragment[^\n]+) %gl_FragCoord', r'\1', s))

    def test_additional_interface_decoration(self):
        self.reject("display", assembly=lambda s: s + "\nOpDecorate %gl_FragCoord Flat\n")

    def test_bad_artifact_identifier(self):
        reflection, assembly = self.outputs["original", "display"]
        for name in ("", "a-b", "a b", "1a", "../x"):
            with self.assertRaises(ValueError):
                generate.header(reflection, assembly, bytes(20), b"", name)


if __name__ == "__main__":
    unittest.main()
