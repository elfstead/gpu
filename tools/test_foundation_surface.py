"""Regression checks for the bounded C-declarator audit, especially pointer constness."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('surface', Path(__file__).with_name('check-foundation-surface.py'))
surface = importlib.util.module_from_spec(spec)
spec.loader.exec_module(surface)


class SignatureTests(unittest.TestCase):
    def test_pointer_qualification_and_fixed_extent(self):
        types = {'thing': 'Thing', 'void': '()', 'float': 'f32', 'uint32_t': 'u32'}
        for c, rust in (
            ('const thing *input', '*const Thing'),
            ('thing *const *inputs', '*const *mut Thing'),
            ('const thing **output', '*mut *const Thing'),
            ('void **output', '*mut *mut std::ffi::c_void'),
            ('uint32_t count', 'u32'),
            ('const float values[4]', '*const [f32; 4]'),
        ):
            self.assertEqual(surface.rust_type(c, types), rust)
        for invalid in ('unrecognized *value', 'thing value[2][3]', 'thing (*callback)(void)', 'thing *restrict value'):
            with self.assertRaises(ValueError):
                surface.rust_type(invalid, types)

    def test_header_operations_and_duplicates(self):
        header = '''typedef struct ogpu_next_presentation_functions {
          void (*destroy)(thing *);
        } ogpu_next_presentation_functions;
        const thing *ogpu_next_lookup(uint32_t index);
        // void ogpu_next_ignored(void);
        '''
        ops = surface.declarations(surface.strip_comments(header))
        self.assertEqual({n for n, _, _ in ops}, {'ogpu_next_lookup', 'presentation.destroy'})
        code = '\n'.join(surface.signatures(ops, {'thing': 'Thing', 'uint32_t': 'u32', 'void': '()'}))
        self.assertIn('fn(u32) -> *const Thing = api::ogpu_next_lookup', code)
        self.assertIn('Option<unsafe extern "C" fn(*mut Thing) -> ()> = table.destroy', code)
        with self.assertRaises(ValueError):
            surface.declarations(header + '\nconst thing *ogpu_next_lookup(uint32_t index);')


if __name__ == '__main__':
    unittest.main()
