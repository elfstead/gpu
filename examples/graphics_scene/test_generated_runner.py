import importlib.util
import json
from pathlib import Path
import unittest
import struct
import handoff

path=Path(__file__).resolve().parents[2]/'tools/sdk/indexed-scene/run.py'
spec=importlib.util.spec_from_file_location('installed_scene',path)
scene=importlib.util.module_from_spec(spec);spec.loader.exec_module(scene)


class InstalledSceneTests(unittest.TestCase):
    def test_embedded_artifact_extraction(self):
        header='static const uint32_t sample_code[] = {0x07230203u,0x00010500u,0x00000000u,0x00000001u,0x00000000u};'
        self.assertEqual(handoff.binary(header,'sample'),struct.pack('<5I',0x07230203,0x10500,0,1,0))
        for bad in ('',header+header,header.replace('07230203','00000000')):
            with self.assertRaises(ValueError):handoff.binary(bad,'sample')

    def test_profile_and_frame_rejections(self):
        device=dict(vendor=4098,device=29471,api=[1,4,354])
        frames=[dict(mode=m,phase=int(f==1),frame=f) for m in range(10) for f in range(3)]
        for bits in (16,32):
            public=dict(abi=19,index_bytes=bits//8,gpu_generated=True)
            generated=dict(compute_push_bytes=32,vertex_push_bytes=8)
            def log():
                return ('DEVICE '+json.dumps(device)+'\nPUBLIC_SCENE '+json.dumps(public)+
                    '\nGENERATED_SCENE '+json.dumps(generated)+'\n'+''.join('SCENE_FRAME '+json.dumps(f)+'\n' for f in frames)+
                    'Public indexed/depth frames drained; CPU oracle must independently accept outputs')
            self.assertEqual(scene.check_logs(log(),'',bits,19),(device,frames))
            for target,key in ((public,'abi'),(public,'index_bytes'),(generated,'compute_push_bytes'),
                               (generated,'vertex_push_bytes'),(frames[1],'phase'),(frames[1],'frame')):
                target[key]+=1
                with self.assertRaises(ValueError):scene.check_logs(log(),'',bits,19)
                target[key]-=1
            for bad in (log().replace('GENERATED_SCENE ','HIDDEN '),log().replace('frames drained','not drained'),
                        log()+'\nDEVICE '+json.dumps(device),log().replace('SCENE_FRAME ','IGNORED ',1)):
                with self.assertRaises(ValueError):scene.check_logs(bad,'',bits,19)
            for error in ('Validation Error: injected','SYNC-HAZARD injected'):
                with self.assertRaises(ValueError):scene.check_logs(log(),error,bits,19)


if __name__=='__main__':unittest.main()
