"""Actual paired compiler output, legal variations and rejected interface drift."""
import copy
import json
import os
import re
import struct
import unittest
import generate as g
import link_graphics
from stage_workflow import VARIANTS


class StageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixtures = {}
        for variant in VARIANTS:
            for stage in ("vertex", "fragment"):
                path = g.ROOT / "target/compiler-stages" / variant / stage
                cls.fixtures[variant, stage] = (json.loads((path / "reflection.json").read_text()),
                    g.run("spirv-dis", str(path / "transform.spv")), (path / "transform.spv").read_bytes())

    def fixture(self, stage, variant="original"):
        return self.fixtures[variant, stage]

    def description(self, stage, variant="original"):
        return g.inspect(*self.fixture(stage, variant)[:2])[0].io

    def reject(self, stage, mutate=None, native=None):
        reflection, assembly, _ = self.fixture(stage)
        reflection = copy.deepcopy(reflection)
        if mutate: mutate(reflection)
        if native: assembly = native(assembly)
        with self.assertRaises((ValueError, KeyError)): g.inspect(reflection, assembly)

    def test_all_pairs(self):
        for variant in VARIANTS:
            inputs = g.stages.link(self.description("vertex", variant), self.description("fragment", variant))
            self.assertEqual(len(inputs), 2)
            self.assertEqual({(r["scalar"], r["lanes"]) for r in inputs}, {("float32", 2), ("uint32", 1)})

    def test_reordered_locations(self):
        original = self.description("fragment")
        reordered = self.description("fragment", "reordered")
        self.assertNotEqual(original, reordered)
        self.assertEqual(original[0]["scalar"], "float32")
        self.assertEqual(reordered[0]["scalar"], "uint32")
        with self.assertRaisesRegex(ValueError, "type mismatch"):
            g.stages.link(self.description("vertex"), reordered)

    def test_names_do_not_link(self):
        self.assertEqual(self.description("fragment"), self.description("fragment", "renamed"))

    def test_fragment_owns_interpolation(self):
        fragment = self.description("fragment", "noperspective")
        self.assertEqual(fragment[0]["interpolation"], "NoPerspective")
        self.assertEqual(self.description("vertex")[0]["interpolation"], "Smooth")
        g.stages.link(self.description("vertex"), fragment)

    def test_extra_vertex_output_allowed(self):
        vertex = self.description("vertex")
        extra = dict(vertex[0]); extra["location"] = 7
        g.stages.link(vertex+[extra], self.description("fragment"))

    def test_missing_producer(self):
        vertex = self.description("vertex")
        with self.assertRaisesRegex(ValueError, "no vertex producer"):
            g.stages.link(vertex[1:], self.description("fragment"))

    def test_width_and_scalar_mismatch(self):
        for key, value in (("lanes", 3), ("scalar", "uint32")):
            vertex = self.description("vertex")
            vertex[0][key] = value
            with self.assertRaisesRegex(ValueError, "type mismatch"):
                g.stages.link(vertex, self.description("fragment"))

    def test_reflection_field_type(self):
        self.reject("vertex", mutate=lambda r: r["entryPoints"][0]["result"]["type"]["fields"][1]["type"].update(elementCount=3))

    def test_reflection_binding(self):
        self.reject("vertex", mutate=lambda r: r["entryPoints"][0]["result"]["binding"].update(count=3))
        self.reject("fragment", mutate=lambda r: r["entryPoints"][0]["parameters"][0]["type"]["fields"][2]["binding"].update(index=0))

    def test_semantics_and_nested_types(self):
        self.reject("vertex", mutate=lambda r: r["entryPoints"][0]["result"]["type"]["fields"][1].update(semanticName="SV_CLIPDISTANCE"))
        self.reject("fragment", mutate=lambda r: r["entryPoints"][0]["parameters"][0]["type"]["fields"][1]["type"].update(kind="struct", fields=[]))

    def test_native_locations(self):
        self.reject("vertex", native=lambda a: a.replace("Location 1", "Location 0"))
        self.reject("fragment", native=lambda a: a.replace("Location 1", "Location 2"))

    def test_native_interpolation(self):
        self.reject("fragment", native=lambda a: re.sub(r"^.*OpDecorate %input_tag Flat\n", "", a, flags=re.M))
        self.reject("fragment", native=lambda a: a.replace("%input_tag Flat", "%input_tag Centroid"))
        self.reject("vertex", native=lambda a: a+"\n OpDecorate %gl_Position Flat\n")
        self.reject("fragment", native=lambda a: a+"\n OpDecorate %input_uv Component 1\n")

    def test_native_width_and_builtin(self):
        self.reject("vertex", native=lambda a: a.replace("OpTypeVector %float 2", "OpTypeVector %float 3"))
        self.reject("fragment", native=lambda a: a.replace("BuiltIn FragCoord", "BuiltIn PointCoord"))

    def test_generated_pair_keeps_native_words(self):
        header = (g.HERE / "pattern.generated.h").read_text()
        for stage in ("vertex", "fragment"):
            section = re.search(rf"pattern_{stage}_code\[\] = \{{(.*?)\}};", header, re.S)[1]
            words = [int(s, 16) for s in re.findall(r"0x([0-9a-f]+)u", section)]
            self.assertEqual(struct.pack(f"<{len(words)}I", *words), self.fixture(stage)[2])

    def test_real_mismatched_pair_rejects_before_header(self):
        with self.assertRaisesRegex(ValueError, "type mismatch"):
            link_graphics.header(g.HERE / "stage_vertex.slang",
                g.ROOT / "target/compiler-stages/reordered/stage_fragment.slang",
                g.ROOT / "target/compiler-stages/mismatch", os.getenv("SLANGC", "slangc"), "bad")


if __name__ == "__main__": unittest.main()
