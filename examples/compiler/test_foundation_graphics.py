"""Check actual linked artifacts, shared bank records and mismatched-stage rejection."""
import json
import os
import subprocess
import shlex
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

    def test_multiview_artifacts_and_legacy_rejection(self):
        directory = g.ROOT / "target/foundation-graphics/multiview"
        directory.mkdir(parents=True, exist_ok=True)
        for stage, source in (
            ("vertex", (g.ROOT / "examples/shaders/foundation-multiview.slang").read_text().replace("vertexMain", "main")),
            ("fragment", '[shader("fragment")] float4 main(uint view : SV_ViewID) : SV_Target { return float4(view,0,0,1); }'),
        ):
            path = directory / (stage+".slang"); path.write_text(source)
            reflected, assembly, binary = g.compile_source(path, directory / stage, os.getenv("SLANGC", "slangc"), stage=stage)
            content = g.header(reflected, assembly, binary, source.encode(), api="foundation")
            self.assertIn("OGPU_NEXT_FEATURE_MULTIVIEW", content)
            with self.assertRaisesRegex(ValueError, "multiview requires"):
                g.header(reflected, assembly, binary, source.encode(), api="installed")
            with self.assertRaisesRegex(ValueError, "view identity"):
                g.inspect(reflected, assembly.replace("OpCapability MultiView", ""))

    def test_shading_rate_native_input_alternatives(self):
        directory = g.ROOT / "target/foundation-graphics/shading-rate"
        for stage in ("vertex", "fragment"):
            path = g.HERE / ("shading_rate_"+stage+".slang")
            r, a, b = g.compile_source(path, directory / stage, os.getenv("SLANGC", "slangc"), stage=stage)
            content = g.header(r, a, b, path.read_bytes(), name="rate", api="foundation")
            with self.assertRaisesRegex(ValueError, "shading rate requires"):
                g.header(r, a, b, path.read_bytes(), api="installed")
            with self.assertRaisesRegex(ValueError, "shading-rate capability"):
                g.inspect(r, a.replace("OpCapability FragmentShadingRateKHR", ""))
            folder = directory / stage
            (folder / "rate.h").write_text(content)
            # Run the generated host predicate: each independent input suffices
            # for fragment ShadingRate, only primitive output suffices for vertex.
            (folder / "check.c").write_text('#include "rate.h"\nint main(void) {\n'
                'ogpu_next_execution_limits l={0}; ogpu_next_feature ids[2]={OGPU_NEXT_FEATURE_RASTER,0};\n'
                'ogpu_next_feature_info f={.enabled={ids,2,0},.baseline_supported=1};\n'
                'if (rate_compatible(&f,&l)) return 1;\n'
                'const ogpu_next_feature inputs[3]={OGPU_NEXT_FEATURE_PIPELINE_SHADING_RATE,OGPU_NEXT_FEATURE_PRIMITIVE_SHADING_RATE,OGPU_NEXT_FEATURE_ATTACHMENT_SHADING_RATE};\n'
                'for (int i=0;i<3;++i) { ids[1]=inputs[i]; if (rate_compatible(&f,&l) != '+('1' if stage == 'fragment' else '(i==1)')+') return 2; }\nreturn 0; }\n')
            binary = folder / "check"
            subprocess.run([*shlex.split(os.getenv("CC", "cc")), "-std=c11", "-Wall", "-Wextra", "-Werror", "-pedantic",
                            "-I"+str(g.ROOT/"include"), str(folder/"check.c"), "-o", str(binary)], check=True)
            subprocess.run([str(binary)],check=True)

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
        for expected in ("pattern_push_size = 8", "OGPU_NEXT_ARGUMENT_GRAPHICS, pattern_push_size",
                         "pattern_vertex_feature_ids, pattern_fragment_feature_ids",
                         "static const ogpu_next_argument_layout_desc pattern_arguments",
                         ".local_size = {0, 0, 0}"):
            self.assertIn(expected,text)

    def test_feature_union(self):
        r,a,b = self.fixtures["original","fragment"]
        text = g.header(r,a+"\nOpCapability Float16\n",b,b"synthetic",api="foundation")
        self.assertIn("OGPU_NEXT_FEATURE_FLOAT16, OGPU_NEXT_FEATURE_RASTER",text)
        self.assertNotIn("max_local_invocations",text)


if __name__ == "__main__": unittest.main()
