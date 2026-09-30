"""Negative tests for full native/public output comparison, independent of a GPU."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec=importlib.util.spec_from_file_location('scene_runner',Path(__file__).with_name('run.py'))
runner=importlib.util.module_from_spec(spec);spec.loader.exec_module(runner)


class ComparisonTests(unittest.TestCase):
    def test_index_width_comparison(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);old=root/'old';new=root/'new'
            old.mkdir();new.mkdir()
            name='mode-0-frame-0.images'
            (old/name).write_bytes(b'expected');(new/name).write_bytes(b'expected')
            reference=dict(directory='old',files={name:runner.digest(old/name)})
            rows=[dict(mode=0,phase=0,frame=0)]
            self.assertEqual(runner.compare_index_width(root,reference,new,rows),dict(frames=1,files=1,compared_bytes=8,exact=True))
            (new/name).write_bytes(b'changed')
            with self.assertRaisesRegex(ValueError,'index-width image mismatch'):
                runner.compare_index_width(root,reference,new,rows)
            (old/name).write_bytes(b'changed')
            with self.assertRaisesRegex(ValueError,'reference artifact hash mismatch'):
                runner.compare_index_width(root,reference,new,rows)

    def test_exact_and_mutated_outputs(self):
        with tempfile.TemporaryDirectory() as directory:
            native=Path(directory)/'native';public=Path(directory)/'public'
            native.mkdir();public.mkdir()
            rows=[dict(mode=0,phase=0,frame=0),dict(mode=0,phase=1,frame=1)]
            for row in rows:
                for suffix in ('images','geometry'):
                    name=f"mode-{row['mode']}-frame-{row['frame']}.{suffix}"
                    (native/name).write_bytes(bytes(range(32)))
                    (public/name).write_bytes(bytes(range(32)))
            self.assertEqual(runner.compare_outputs(native,public,rows),dict(frames=2,files=4,compared_bytes=128,exact=True))
            for suffix in ('images','geometry'):
                target=public/f'mode-0-frame-1.{suffix}'
                for bad in (bytes(range(31)),bytes([255])+bytes(range(1,32)),bytes(range(31))+bytes([255])):
                    target.write_bytes(bad)
                    with self.assertRaisesRegex(ValueError,'public/native byte mismatch'):
                        runner.compare_outputs(native,public,rows)
                target.write_bytes(bytes(range(32)))
            (public/'mode-0-frame-0.images').unlink()
            with self.assertRaises(FileNotFoundError): runner.compare_outputs(native,public,rows)


if __name__=='__main__': unittest.main()
