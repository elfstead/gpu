"""Checked native draw identity, address-based vertex roots and rootless fragments."""
import copy
import json
import os
import re
import unittest
import generate as g


class RangeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixtures = {}
        for stage in ('vertex', 'fragment'):
            path = g.ROOT / 'target/compiler-ranges' / stage
            cls.fixtures[stage] = (json.loads((path/'reflection.json').read_text()),
                                  g.run('spirv-dis', str(path/'transform.spv')))

    def reject(self, stage='vertex', reflected=None, native=None):
        r, a = self.fixtures[stage]
        r = copy.deepcopy(r)
        if reflected: reflected(r)
        if native: a = native(a)
        with self.assertRaises((ValueError, KeyError)): g.inspect(r, a)

    def test_real_requirements_and_root(self):
        fields, size, alignment, local, caps = g.inspect(*self.fixtures['vertex'], 'identity_vertex')
        self.assertEqual((size, alignment, local), (8, 8, []))
        self.assertEqual([(f[0], f[2]) for f in fields], [('arg_vertices', 0)])
        self.assertEqual(caps, ['buffer_device_address', 'graphics_queue', 'shader_draw_parameters'])
        f, size, alignment, local, caps = g.inspect(*self.fixtures['fragment'])
        self.assertEqual((len(f), size, alignment, local, caps), (0, 0, 1, [], ['graphics_queue']))

    def test_unknown_and_duplicate_ids(self):
        self.reject(reflected=lambda r: r['entryPoints'][0]['parameters'][1].update(semanticName='SV_INSTANCEID'))
        self.reject(reflected=lambda r: r['entryPoints'][0]['parameters'][1].update(semanticName='SV_VULKANVERTEXID'))
        self.reject(reflected=lambda r: r['entryPoints'][0]['parameters'].pop())
        self.reject(native=lambda a: a.replace('BuiltIn DrawIndex', 'BuiltIn BaseInstance'))

    def test_missing_or_wrong_stage_capability(self):
        self.reject(native=lambda a: re.sub(r'^.*OpCapability DrawParameters\n', '', a, flags=re.M))
        self.reject('fragment', native=lambda a: a+'\nOpCapability DrawParameters\n')
        self.reject(native=lambda a: a.replace('BuiltIn DrawIndex', 'Location 7'))

    def test_root_disagreement(self):
        self.reject(native=lambda a: a.replace('0 Offset 0', '0 Offset 4'))
        def remove(r):
            r['parameters'] = []
            r['entryPoints'][0]['bindings'] = []
        self.reject(reflected=remove)

    def test_generated_predicate(self):
        r, a = self.fixtures['vertex']
        binary = (g.ROOT/'target/compiler-ranges/vertex/transform.spv').read_bytes()
        header = g.header(r, a, binary, b'', 'identity_vertex')
        self.assertIn('c->shader_draw_parameters', header)
        self.assertIn('l->max_push_data_bytes >= 8', header)

    def test_real_vertex_storage_write_rejected(self):
        # Compile/validate, never execute this intentionally unsupported stage.
        folder = g.ROOT/'target/compiler-ranges/storage-write'
        folder.mkdir(parents=True, exist_ok=True)
        source = folder/'vertex.slang'
        text = (g.ROOT/'examples/graphics_scene/identity.vert.slang').read_text()
        source.write_text(text.replace('    Varyings result;', '    root.vertices[0] = 1.0;\n    Varyings result;'))
        reflection, assembly, _ = g.compile_source(source, folder, os.getenv('SLANGC', 'slangc'), 'vertex')
        with self.assertRaisesRegex(ValueError, 'graphics storage writes'):
            g.inspect(reflection, assembly)

    def test_graphics_effects_do_not_escape_feature_checks(self):
        definitions = {'%float':['OpTypeFloat','32'], '%local':['OpTypePointer','Function','%float'],
                       '%physical':['OpTypePointer','PhysicalStorageBuffer','%float'],
                       '%a':['OpVariable','%local','Function'], '%b':['OpFunctionParameter','%physical']}
        g.stages.check_writes(definitions, 'OpStore %a %value')
        for instruction in ('OpStore %b %value', 'OpCopyMemory %b %a', 'OpCopyMemorySized %b %a %size',
                            'OpImageWrite %image %coord %value', '%result = OpAtomicLoad %uint %b %scope %memory'):
            with self.assertRaisesRegex(ValueError, 'graphics storage writes'):
                g.stages.check_writes(definitions, instruction)


if __name__ == '__main__': unittest.main()
