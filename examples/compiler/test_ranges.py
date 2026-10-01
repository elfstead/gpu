"""Checked native draw identity, address-based vertex roots and rootless fragments."""
import copy
import json
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


if __name__ == '__main__': unittest.main()
