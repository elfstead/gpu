"""Receipt/comparison rejections; pixel mathematics has separate oracle tests."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec=importlib.util.spec_from_file_location('textured_control',Path(__file__).with_name('textured.py'))
runner=importlib.util.module_from_spec(spec);spec.loader.exec_module(runner)


class ReceiptTests(unittest.TestCase):
    def test_complete_and_mutated_artifacts(self):
        with tempfile.TemporaryDirectory() as directory:
            folder=Path(directory)
            for mode in range(6):
                for frame in range(3):
                    phase=int(frame==1)
                    stem=f'mode-{mode}-frame-{frame}'
                    (folder/(stem+'.images')).write_bytes(bytes((1 if mode==2 else mode,phase)))
                    (folder/(stem+'.geometry')).write_bytes(runner.oracle.geometry(phase,False))
                    (folder/(stem+'.texture')).write_bytes(runner.texture.GUARD+runner.texture.texture(phase)+bytes([0xa5])*108)
            with patch.object(runner.texture,'check',return_value=dict(checked_pixels=100)) as pixels:
                self.assertEqual(len(runner.check_output(folder,257,193)),18)
                self.assertEqual(pixels.call_count,18)
                for name in ('mode-0-frame-0.geometry','mode-0-frame-1.texture','mode-0-frame-2.images','mode-2-frame-1.images'):
                    path=folder/name;good=path.read_bytes();path.write_bytes(b'bad')
                    with self.assertRaises(ValueError):runner.check_output(folder,257,193)
                    path.write_bytes(good)
                (folder/'mode-5-frame-2.images').unlink()
                with self.assertRaises(FileNotFoundError):runner.check_output(folder,257,193)

    def test_texture_failure_lifetimes(self):
        cleanup='TEARDOWN arena\n'+'TEARDOWN memory\n'*12+'TEARDOWN device\n'
        for mode in ('end','submit','unknown','wait'):
            known=mode in ('end','submit')
            log='INJECT '+mode+'\n'+(cleanup if known else 'Unknown completion; refusing unsafe teardown\n')
            def check(code,out,err):runner.scene_control.check_failure(mode,code,out,err,12,'TEXTURED_FRAME ')
            check(1,'',log)
            for code,out,err in ((0,'',log),(1,'TEXTURED_FRAME {}',log),(1,'',log+'Validation Error:'),
                                 (1,'',log.replace('TEARDOWN memory\n','',1) if known else log+cleanup)):
                with self.assertRaises(ValueError):check(code,out,err)


if __name__=='__main__':unittest.main()
