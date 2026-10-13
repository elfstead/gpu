"""Independent SDK receipt lifecycle checks without a compiler or GPU."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
sys.dont_write_bytecode=True
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'examples/graphics_scene'))
spec=importlib.util.spec_from_file_location('textured_sdk',ROOT/'tools/sdk/foundation-textured/run.py')
sdk=importlib.util.module_from_spec(spec);spec.loader.exec_module(sdk)


def log(device=1):
    return ('DEVICE '+json.dumps(dict(vendor=1,device=device,api=[1,4,0]))+'\n'+''.join(
        'TEXTURED_FRAME '+json.dumps(dict(mode=m,frame=f,phase=int(f==1)))+'\n' for m in range(6) for f in range(3))+
        'Foundation textured frames drained; independent oracle required\n')


class ReceiptTests(unittest.TestCase):
    def run_case(self,outputs,okay=False,accepted=0,oracle_error=False):
        with tempfile.TemporaryDirectory() as directory:
            here=Path(directory);(here/'consumer').write_bytes(b'fixture executable')
            (here/'libogpu.so').write_bytes(b'fixture runtime');(here/'ogpu_next.h').write_text('fixture header')
            def variable(command,**kwargs):
                key=command[1].removeprefix('--variable=')
                return {'ogpu_foundation':'1','ogpu_revision':'test-revision','libdir':str(here),'includedir':str(here)}[key]
            def run(command,**kwargs):
                code,stdout,stderr=outputs.pop(0)
                return subprocess.CompletedProcess(command,code,stdout,stderr)
            with patch.object(sdk,'HERE',here),patch.object(sys,'argv',['run.py']),\
                 patch.object(sdk.subprocess,'check_output',side_effect=variable),\
                 patch.object(sdk.subprocess,'run',side_effect=run),\
                 patch.object(sdk,'check_output',side_effect=ValueError('corrupt output') if oracle_error else None,return_value=[{}]*18):
                if okay:sdk.main()
                else:
                    with self.assertRaises(ValueError):sdk.main()
            paths=list(here.glob('run-*/report.json'));self.assertEqual(len(paths),1)
            report=json.loads(paths[0].read_text())
            self.assertEqual(report['complete'],okay);self.assertEqual(len(report['runs']),accepted)
            self.assertEqual(report['sdk']['revision'],'test-revision')
            self.assertEqual('error' in report,not okay)

    def test_complete(self):self.run_case([(0,log(),''),(0,log(),'')],okay=True,accepted=2)

    def test_rejections_leave_incomplete_receipts(self):
        for result in ((1,'','missing loader'),(0,log(),'Validation Error: bad'),(0,log(),'SYNC-HAZARD'),
                       (0,log().replace('TEXTURED_FRAME','HIDDEN',1),''),(0,log().replace('frames drained','not drained'),''),
                       (0,log()+'DEVICE {}\n','')):
            with self.subTest(result=result):self.run_case([result])
        self.run_case([(0,log(),'')],oracle_error=True)
        self.run_case([(0,log(),''),(0,log(2),'')],accepted=1)


if __name__=='__main__':unittest.main()
