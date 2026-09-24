"""Cross-check actual pinned compiler heap output; reject unverified interfaces."""
import json
import os
import re
import unittest
import generate as g


class HeapTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixtures = {}
        for variant in ("original", "reordered"):
            for name in ("heap-process", "heap-sample"):
                path = g.ROOT / "target/compiler-heaps" / variant / name
                cls.fixtures[variant, name] = (json.loads((path / "reflection.json").read_text()),
                    g.run("spirv-dis", str(path / "transform.spv")), (path / "transform.spv").read_bytes())

    def fixture(self, name="heap-process", variant="original"):
        return self.fixtures[variant, name]

    def reject(self, assembly, name="heap-process"):
        with self.assertRaises(ValueError): g.inspect(self.fixture(name)[0], assembly)

    def test_requirements(self):
        for name, queue, size, resources in (
            ("heap-process", "compute_queue", 16, ["sampled_image_2d_float", "storage_image_2d_rgba8"]),
            ("heap-sample", "graphics_queue", 20, ["sampled_image_2d_float", "sampler"])):
            reflection, assembly, _ = self.fixture(name)
            fields, actual_size, align, _, requirements = g.inspect(reflection, assembly)
            self.assertEqual((actual_size, align), (size, 4))
            self.assertEqual(requirements, sorted([queue, "descriptor_heap", "shader_untyped_pointers"]))
            self.assertEqual(fields.resources, resources)

    def test_reordered(self):
        for name in ("heap-process", "heap-sample"):
            original = g.inspect(*self.fixture(name)[:2])
            changed = g.inspect(*self.fixture(name, "reordered")[:2])
            self.assertEqual([f[0] for f in changed[0]], list(reversed([f[0] for f in original[0]])))
            self.assertEqual(original[1:3], changed[1:3])
            if name == "heap-process": self.assertEqual(changed[3], [32, 1, 1])

    def test_generated_requirements(self):
        reflection, assembly, binary = self.fixture("heap-sample")
        header = g.header(reflection, assembly, binary, b"fixture", "sample")
        for text in ("sample_uses_sampler = 1", "c->descriptor_heap", "c->shader_untyped_pointers",
                     "c->graphics_queue", "l->max_push_data_bytes >= 20", "float arg_offsetX"):
            self.assertIn(text, header)

    def test_caps_and_extensions(self):
        assembly = self.fixture()[1]
        for text in ("OpCapability DescriptorHeapEXT", "OpCapability UntypedPointersKHR",
                     'OpExtension "SPV_EXT_descriptor_heap"', 'OpExtension "SPV_KHR_untyped_pointers"'):
            self.reject(assembly.replace(text, ""))
        self.reject(assembly+'\n OpCapability Int64\n')
        self.reject(assembly+'\n OpExtension "SPV_unknown_extension"\n')

    def test_descriptor_set_rejected(self):
        source = g.ROOT / "examples/shaders/heap-process.slang"
        reflection, assembly, _ = g.compile_source(source, g.ROOT / "target/compiler-heaps/descriptor-set",
                                                   os.getenv("SLANGC", "slangc"))
        self.assertIn("DescriptorSet", assembly)
        with self.assertRaises(ValueError): g.inspect(reflection, assembly)

    def test_heap_swap(self):
        self.reject(self.fixture()[1].replace("BuiltIn ResourceHeapEXT", "BuiltIn SamplerHeapEXT"))
        self.reject(self.fixture("heap-sample")[1].replace("BuiltIn SamplerHeapEXT", "BuiltIn ResourceHeapEXT"), "heap-sample")

    def test_image_shape(self):
        assembly = self.fixture()[1]
        for old, new in ((" 2D 2 0 0 1 Unknown", " 3D 2 0 0 1 Unknown"),
                         (" 2D 2 0 0 1 Unknown", " 2D 2 1 0 1 Unknown"),
                         (" 2D 2 0 0 1 Unknown", " 2D 2 0 1 1 Unknown"),
                         ("OpTypeFloat 32", "OpTypeFloat 16"), (" Rgba8", " Rgba16f")):
            self.assertIn(old, assembly); self.reject(assembly.replace(old, new))

    def test_descriptor_stride(self):
        assembly = self.fixture()[1]
        self.reject(assembly.replace("ArrayStrideIdEXT", "ArrayStride"))
        self.reject(re.sub(r"OpConstantSizeOfEXT (%\S+) %\S+", r"OpConstant \1 16", assembly))
        sizes = re.findall(r"OpConstantSizeOfEXT %\S+ (%\S+)", assembly)
        self.assertEqual(len(sizes), 2)
        self.reject(assembly.replace("OpConstantSizeOfEXT %uint "+sizes[0], "OpConstantSizeOfEXT %uint "+sizes[1]))

    def test_untyped_path(self):
        assembly = self.fixture()[1]
        self.reject(assembly.replace("OpUntypedAccessChainKHR", "OpUntypedInBoundsAccessChainKHR"))
        self.reject(assembly.replace("OpTypeUntypedPointerKHR UniformConstant", "OpTypeUntypedPointerKHR StorageBuffer"))
        self.reject(assembly.replace("OpUntypedVariableKHR", "OpVariable"))

    def test_extra_decoration(self):
        assembly = self.fixture()[1]
        stride = re.search(r"OpDecorateId (%\S+) ArrayStrideIdEXT (%\S+)", assembly)
        self.reject(assembly+"\n "+stride[0]+"\n")
        self.reject(assembly+f"\n OpDecorate {stride[1]} ArrayStride 16\n")
        self.reject(assembly+"\n OpDecorate %slang_resourceHeap Binding 0\n")

    def test_pointer_escape(self):
        assembly = self.fixture()[1]
        self.reject(assembly.replace("OpReturn", "OpReturnValue %slang_resourceHeap"))
        self.reject(assembly.replace("OpReturn", "%escaped = OpCopyObject %_ptr_UniformConstant %slang_resourceHeap\n OpReturn"))
        pointer = re.search(r"(%\S+) = OpUntypedAccessChainKHR", assembly)[1]
        self.reject(assembly.replace("OpReturn", "OpReturnValue "+pointer))


if __name__ == "__main__": unittest.main()
