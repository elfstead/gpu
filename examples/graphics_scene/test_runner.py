"""Negative tests for full native/public output comparison, independent of a GPU."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace

spec=importlib.util.spec_from_file_location('scene_runner',Path(__file__).with_name('run.py'))
runner=importlib.util.module_from_spec(spec);spec.loader.exec_module(runner)


class ComparisonTests(unittest.TestCase):
    def test_failure_receipts(self):
        for mode in ('end','submit','unknown','wait'):
            known=mode in ('end','submit')
            cleanup='TEARDOWN arena\n'+'TEARDOWN memory\n'*8+'TEARDOWN device\n'
            log=f'INJECT {mode}\n'+(cleanup if known else 'Unknown foundation scene completion; refusing unsafe teardown\n')
            runner.check_failure(mode,1,'',log)
            for code,stdout,stderr in ((0,'',log),(1,'SCENE_FRAME {}',log),(1,'',log+'Validation Error:'),
                                      (1,'',log.replace('INJECT','MISSED')),
                                      (1,'',log.replace('TEARDOWN arena','TEARDOWN other') if known else log+cleanup)):
                with self.assertRaises(ValueError):runner.check_failure(mode,code,stdout,stderr)

    def test_foundation_requirements(self):
        fields=SimpleNamespace(io=['fixture'])
        for stage,size,local,requirements in (
            ('compute',32,[8,1,1],['compute_queue','buffer_device_address']),
            ('vertex',8,[],['graphics_queue','buffer_device_address']),
            ('fragment',0,[],['graphics_queue'])):
            with patch.object(runner.generate,'inspect',return_value=(fields,size,8,local,requirements)):
                self.assertEqual(runner.inspect_foundation_shader({},'',stage),fields.io)
            for bad,message in (
                ((fields,size+4,8,local,requirements),'root size'),
                ((fields,size,8,[1,1,1],requirements),'local size'),
                ((fields,size,8,local,requirements+['shader_float16']),'requirements changed')):
                with patch.object(runner.generate,'inspect',return_value=bad):
                    with self.assertRaisesRegex(ValueError,message):runner.inspect_foundation_shader({},'',stage)

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
