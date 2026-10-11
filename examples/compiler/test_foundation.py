"""Foundation emission reuses the installed adapter's checked artifact contract."""
import json
import os
import sys
import unittest
sys.dont_write_bytecode = True
import generate


class FoundationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        directory = generate.ROOT / "target/foundation-compiler/original"
        cls.reflection = json.loads((directory / "reflection.json").read_text())
        cls.assembly = generate.run("spirv-dis", str(directory / "transform.spv"))
        cls.binary = (directory / "transform.spv").read_bytes()
        cls.source = (generate.HERE / "transform.slang").read_bytes()

    def header(self, assembly=None, api="foundation"):
        return generate.header(self.reflection, assembly or self.assembly, self.binary, self.source, api=api)

    def test_same_artifact_and_layout(self):
        installed, foundation = self.header(api="installed"), self.header()
        for text in ('uint64_t arg_data;', 'uint32_t arg_count;', 'offsetof(TransformArguments, arg_data) == 0',
                     'offsetof(TransformArguments, arg_count) == 8', 'transform_local[3] = {64, 1, 1}'):
            self.assertIn(text, installed)
            self.assertIn(text, foundation)
        artifact = lambda text: text.split('static const uint32_t transform_code[] = {')[1].split('};')[0]
        self.assertEqual(artifact(installed), artifact(foundation))
        self.assertIn('#include "ogpu_next.h"', foundation)
        self.assertIn('.local_size = {64, 1, 1}', foundation)
        self.assertIn('.byte_size = 16, .root_count = 0', foundation)
        self.assertIn('transform_required_features (UINT64_C(0))', foundation)
        self.assertNotIn('OgpuShaderDesc', foundation)

    def test_optional_feature_uses_enabled_mask(self):
        # Synthetic capability metadata, not an FP16 shader execution claim.
        text = self.header(self.assembly+'\nOpCapability Float16\n')
        self.assertIn('transform_required_features (OGPU_NEXT_FEATURE_FLOAT16)', text)
        self.assertIn('c->enabled & transform_required_features', text)
        self.assertNotIn('c->available', text)

    def test_existing_rejection_is_not_bypassed(self):
        for suffix in ('\nOpCapability Int64\n', '\nOpExtension "SPV_unknown"\n',
                       '\nOpExecutionMode %main SubgroupSize 32\n'):
            with self.assertRaises(ValueError): self.header(self.assembly+suffix)

    def test_wrong_api_rejected(self):
        with self.assertRaises(ValueError): self.header(api="invented")

    def test_graphics_emission_rejected(self):
        source = generate.HERE / "stage_vertex.slang"
        reflected, assembly, binary = generate.compile_source(source,
            generate.ROOT / "target/foundation-compiler/vertex-rejection", os.getenv("SLANGC", "slangc"), "vertex")
        with self.assertRaisesRegex(ValueError, "foundation graphics interface linking"):
            generate.header(reflected, assembly, binary, source.read_bytes(), api="foundation")

    def test_unmapped_requirement_rejected(self):
        with self.assertRaises(ValueError): generate.foundation_records("test",16,[64,1,1],["invented"])


if __name__ == "__main__":
    unittest.main()
