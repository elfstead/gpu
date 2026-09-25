"""Negative lowering checks against newly compiled pinned artifacts (no GPU)."""
import copy
import os
import tempfile
import unittest
from pathlib import Path
import probe


class ProbeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.scratch = tempfile.TemporaryDirectory(prefix="ogpu-language-")
        cls.addClassCleanup(cls.scratch.cleanup)
        cls.artifacts = {name: probe.compile_probe(name, Path(cls.scratch.name), os.getenv("SLANGC", "slangc"))
                         for name in ("reduce", "matmul", "subgroup")}

    def reject(self, name, old, new, message):
        reflection, assembly = self.artifacts[name]
        self.assertIn(old, assembly)
        with self.assertRaisesRegex(ValueError, message):
            probe.inspect(name, reflection, assembly.replace(old, new))

    def test_originals(self):
        for name, artifact in self.artifacts.items(): probe.inspect(name, *artifact)

    def test_capability(self):
        self.reject("reduce", "OpCapability Shader", "OpCapability Shader\nOpCapability Int64", "capability")

    def test_extension(self):
        self.reject("reduce", 'OpExtension "SPV_KHR_physical_storage_buffer"',
                    'OpExtension "SPV_KHR_untyped_pointers"', "extension")

    def test_local_size(self):
        self.reject("reduce", "LocalSize 64 1 1", "LocalSize 32 1 1", "local size")

    def test_root_offset(self):
        self.reject("reduce", "2 Offset 16", "2 Offset 12", "offset|layout")

    def test_pointer_stride(self):
        self.reject("matmul", "ArrayStride 4", "ArrayStride 8", "stride")

    def test_reflection(self):
        reflection, assembly = self.artifacts["matmul"]
        changed = copy.deepcopy(reflection)
        changed["parameters"][0]["type"]["elementVarLayout"]["binding"]["size"] = 24
        with self.assertRaisesRegex(ValueError, "extent"): probe.inspect("matmul", changed, assembly)

    def test_shared_extent(self):
        self.reject("reduce", "%int_64 = OpConstant %int 64", "%int_64 = OpConstant %int 32", "shared extent")

    def test_barrier_scope_and_semantics(self):
        for old, new in (("%uint_2 %uint_2 %uint_264", "%uint_0 %uint_2 %uint_264"),
                         ("%uint_2 %uint_2 %uint_264", "%uint_2 %uint_0 %uint_264"),
                         ("%uint_264 = OpConstant %uint 264", "%uint_264 = OpConstant %uint 256")):
            self.reject("reduce", old, new, "barrier scope/semantics")

    def test_collective_scope(self):
        self.reject("subgroup", "%uint_3 = OpConstant %uint 3", "%uint_3 = OpConstant %uint 2", "collective scope")

    def test_collective_operation(self):
        self.reject("subgroup", "Reduce", "InclusiveScan", "collective scope")

    def test_added_copy_or_barrier(self):
        reflection, assembly = self.artifacts["subgroup"]
        for added in ("OpCopyMemory %x %y", "OpMemoryBarrier %uint_3 %uint_1", "OpFunctionCall %void %callee"):
            with self.assertRaisesRegex(ValueError, "copy/call/barrier"):
                probe.inspect("subgroup", reflection, assembly+"\n"+added+"\n")


if __name__ == "__main__": unittest.main()
