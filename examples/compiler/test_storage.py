"""Actual pinned compiler fixtures plus negative metadata/flow checks."""
import json
import os
import re
import unittest
import generate as g


class StorageTests(unittest.TestCase):
    def test_all_32_bit_scalar_storage_formats_from_pinned_compiler(self):
        # Host/compiler coverage of the whole accepted typed vocabulary, not GPU
        # acceptance of every format. Pinned Slang assigns these formats 1..39.
        found = set()
        for fmt in range(1, 40):
            scalar = "float4" if fmt <= 20 else "int4" if fmt <= 29 else "uint4"
            folder = g.ROOT / "target/compiler-storage/typed-vocabulary" / str(fmt)
            folder.mkdir(parents=True, exist_ok=True)
            source = folder / "storage.slang"
            source.write_text(f"#define TEXEL {scalar}\n#define INPUT_FORMAT {fmt}\n"
                              f"#define OUTPUT_FORMAT {fmt}\n#define OPERATION 1\n"
                              + (g.HERE / "storage.slang").read_text())
            reflection, assembly, _ = g.compile_source(source, folder, os.getenv("SLANGC", "slangc"), native_heaps=True)
            fields, _, _, _, _ = g.inspect(reflection, assembly)
            self.assertEqual(list(fields.image_requirements.values()), [["STORAGE"]])
            found.update(re.findall(r"OpTypeImage .* 2 (\w+)", assembly))
        self.assertEqual(found, g.heaps.STORAGE_FORMATS.keys())

    def fixture(self, label="float", mode=1):
        directory = g.ROOT / "target/compiler-storage" / f"{label}-{mode}"
        return (json.loads((directory / "reflection.json").read_text()),
                g.run("spirv-dis", str(directory / "transform.spv")),
                (directory / "transform.spv").read_bytes())

    def test_operation_not_capability_drives_flags(self):
        for label in ("unorm", "sint", "uint", "float"):
            for mode in range(4):
                reflection, assembly, _ = self.fixture(label, mode)
                fields, _, _, _, _ = g.inspect(reflection, assembly)
                flags = set().union(*map(set, fields.image_requirements.values()))
                self.assertEqual("STORAGE_READ_WITHOUT_FORMAT" in flags, bool(mode & 1))
                self.assertEqual("STORAGE_WRITE_WITHOUT_FORMAT" in flags, bool(mode & 2))
                if mode:
                    self.assertIn("OpCapability StorageImageReadWithoutFormat", assembly)
                    self.assertIn("OpCapability StorageImageWriteWithoutFormat", assembly)

    def test_formatless_flow_fails_closed(self):
        reflection, assembly, _ = self.fixture()
        typ = re.search(r"(%\S+) = OpTypeImage .* 2 Unknown", assembly)[1]
        loaded = re.search(rf"(%\S+) = OpLoad {re.escape(typ)} ", assembly)[1]
        for instruction in (f"OpReturnValue {loaded}", f"%escape = OpCopyObject {typ} {loaded}"):
            with self.assertRaisesRegex(ValueError, "storage descriptor flow"):
                g.inspect(reflection, assembly.replace("OpReturn", instruction))

    def test_typed_storage_does_not_silently_accept_atomic_requirements(self):
        reflection, assembly, _ = self.fixture("uint", 0)
        typ = re.search(r"(%\S+) = OpTypeImage .* 2 Rgba8ui", assembly)[1]
        loaded = re.search(rf"(%\S+) = OpLoad {re.escape(typ)} ", assembly)[1]
        # Flow-check mutation, not a valid atomic shader or image format claim.
        with self.assertRaisesRegex(ValueError, "storage descriptor flow"):
            g.inspect(reflection, assembly.replace("OpReturn", f"%escape = OpImageTexelPointer %ptr {loaded} %coord %sample\n OpReturn"))

    def test_numeric_class_and_shape_checked(self):
        reflection, assembly, _ = self.fixture("sint", 0)
        for old, new in (("Rgba8i", "Rgba8ui"), ("Rgba8i", "Rgba8"),
                         (" 2D 2 0 0 2", " 2D 2 1 0 2")):
            self.assertIn(old, assembly)
            with self.assertRaises(ValueError): g.inspect(reflection, assembly.replace(old, new))

    def test_legacy_rejects_new_capabilities_and_kinds(self):
        for label, mode in (("float", 0), ("unorm", 1), ("sint", 0), ("uint", 0)):
            reflection, assembly, binary = self.fixture(label, mode)
            with self.assertRaisesRegex(ValueError, "foundation API"):
                g.header(reflection, assembly, binary, b"fixture", api="installed")


if __name__ == "__main__": unittest.main()
