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
    def test_reuse_receipt_rejections(self):
        device=dict(vendor=4098,device=29471,api=[1,4,354])
        for bits in (16,32):
            for slots in (1,2):
                for replay in (False,True):
                    profile=dict(foundation=1,index_bytes=bits//8,gpu_generated=True)
                    generated=dict(compute_push_bytes=32,vertex_push_bytes=8)
                    frames=[dict(frame=f,slot=f%slots,generation=f//slots,phase=int(f//slots%4==1),empty=int(f//slots%4==2)) for f in range(8)]
                    summary=dict(frames=8,slots=slots,mode=8,index_bytes=bits//8,replay=int(replay),
                                 encodes=slots if replay else 8,peak_unretired=slots,requested_bytes=slots*(257*193*16+1816))
                    records=[('DEVICE',device),('PUBLIC_SCENE',profile),('GENERATED_SCENE',generated),
                             *[('REUSE_FRAME',f) for f in frames],('REUSE_SUMMARY',summary)]
                    def log():
                        return ''.join(k+' '+json.dumps(v)+'\n' for k,v in records)+'Public scene reuse drained and checked\n'
                    def check(stdout,stderr=''):
                        return scene.check_reuse(stdout,stderr,device,bits,8,slots,replay,257,193)
                    self.assertEqual(check(log()),summary)
                    for target in (profile,generated,*frames,summary):
                        for key in target:
                            saved=target[key];target[key]=int(saved)+1
                            with self.assertRaises(ValueError,msg=key):check(log())
                            target[key]=saved
                    for bad in (log().replace('drained and checked','not drained'),
                                log().replace('REUSE_FRAME ','HIDDEN ',1),
                                log()+'REUSE_SUMMARY '+json.dumps(summary),
                                log().replace('"vendor": 4098','"vendor": 0')):
                        with self.assertRaises(ValueError):check(bad)
                    for error in ('Validation Error: injected','SYNC-HAZARD injected'):
                        with self.assertRaises(ValueError):check(log(),error)

    def test_embedded_artifact_extraction(self):
        header='static const uint32_t sample_code[] = {0x07230203u,0x00010500u,0x00000000u,0x00000001u,0x00000000u};'
        self.assertEqual(handoff.binary(header,'sample'),struct.pack('<5I',0x07230203,0x10500,0,1,0))
        for bad in ('',header+header,header.replace('07230203','00000000')):
            with self.assertRaises(ValueError):handoff.binary(bad,'sample')

    def test_profile_and_frame_rejections(self):
        device=dict(vendor=4098,device=29471,api=[1,4,354])
        frames=[dict(mode=m,phase=int(f==1),frame=f) for m in range(10) for f in range(3)]
        for bits in (16,32):
            public=dict(abi=20,index_bytes=bits//8,gpu_generated=True)
            generated=dict(compute_push_bytes=32,vertex_push_bytes=8)
            def log():
                return ('DEVICE '+json.dumps(device)+'\nPUBLIC_SCENE '+json.dumps(public)+
                    '\nGENERATED_SCENE '+json.dumps(generated)+'\n'+''.join('SCENE_FRAME '+json.dumps(f)+'\n' for f in frames)+
                    'Public indexed/depth frames drained; CPU oracle must independently accept outputs')
            self.assertEqual(scene.check_logs(log(),'',bits,20),(device,frames))
            for target,key in ((public,'abi'),(public,'index_bytes'),(generated,'compute_push_bytes'),
                               (generated,'vertex_push_bytes'),(frames[1],'phase'),(frames[1],'frame')):
                target[key]+=1
                with self.assertRaises(ValueError):scene.check_logs(log(),'',bits,20)
                target[key]-=1
            for bad in (log().replace('GENERATED_SCENE ','HIDDEN '),log().replace('frames drained','not drained'),
                        log()+'\nDEVICE '+json.dumps(device),log().replace('SCENE_FRAME ','IGNORED ',1)):
                with self.assertRaises(ValueError):scene.check_logs(bad,'',bits,20)
            for error in ('Validation Error: injected','SYNC-HAZARD injected'):
                with self.assertRaises(ValueError):scene.check_logs(log(),error,bits,20)
            # The shared oracle cannot accept ABI20 logs as foundation evidence,
            # or foundation logs as the installed control.
            with self.assertRaises(ValueError):scene.check_logs(log(),'',bits,1,foundation=True)
            del public['abi'];public['foundation']=1
            self.assertEqual(scene.check_logs(log(),'',bits,1,foundation=True),(device,frames))
            with self.assertRaises(ValueError):scene.check_logs(log(),'',bits,20)
            public['foundation']=2
            with self.assertRaises(ValueError):scene.check_logs(log(),'',bits,1,foundation=True)


if __name__=='__main__':unittest.main()
