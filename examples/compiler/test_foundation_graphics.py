"""Check actual linked artifacts, shared bank records and mismatched-stage rejection."""
import json
import unittest
import generate as g
import link_graphics
from stage_workflow import VARIANTS


class FoundationGraphicsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixtures = {}
        for variant in VARIANTS:
            for stage in ("vertex","fragment"):
                directory = g.ROOT / "target/foundation-graphics" / variant / stage
                cls.fixtures[variant,stage] = (json.loads((directory / "reflection.json").read_text()),
                    g.run("spirv-dis",str(directory / "transform.spv")),(directory / "transform.spv").read_bytes())

    def description(self, variant, stage):
        return g.inspect(*self.fixtures[variant,stage][:2],"pattern_"+stage)

    def test_all_pairs(self):
        for variant in VARIANTS:
            vertex,fragment = [self.description(variant,s) for s in ("vertex","fragment")]
            self.assertEqual((vertex[1],fragment[1]),(0,8))
            self.assertEqual(len(g.stages.link(vertex[0].io,fragment[0].io)),2)

    def test_mixed_locations_rejected(self):
        with self.assertRaisesRegex(ValueError,"type mismatch"):
            g.stages.link(self.description("original","vertex")[0].io,self.description("reordered","fragment")[0].io)

    def test_artifact_not_rewritten(self):
        for fixture in self.fixtures.values():
            old,new = [g.header(*fixture,b"fixture",api=api) for api in ("installed","foundation")]
            code = lambda text: text.split("static const uint32_t transform_code[] = {")[1].split("};")[0]
            self.assertEqual(code(old),code(new))

    def test_pair_records(self):
        text = (g.HERE / "foundation-pattern.generated.h").read_text()
        for expected in ("pattern_push_size = 8", ".byte_size = pattern_push_size",
                         "pattern_vertex_required_features | pattern_fragment_required_features",
                         "out[0].interface_metadata = out[1].interface_metadata = &pattern_interface.header",
                         ".local_size = {0, 0, 0}"):
            self.assertIn(expected,text)

    def test_feature_union(self):
        r,a,b = self.fixtures["original","fragment"]
        text = g.header(r,a+"\nOpCapability Float16\n",b,b"synthetic",api="foundation")
        self.assertIn("OGPU_NEXT_FEATURE_FLOAT16 | OGPU_NEXT_FEATURE_RASTER",text)
        self.assertNotIn("max_local_invocations",text)


if __name__ == "__main__": unittest.main()
