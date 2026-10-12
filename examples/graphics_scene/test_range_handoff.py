import importlib.util
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
import oracle
import range_oracle

SDK=Path(__file__).resolve().parents[2]/'tools/sdk'
def load(name,path):
    spec=importlib.util.spec_from_file_location(name,path)
    module=importlib.util.module_from_spec(spec);sys.modules[name]=module;spec.loader.exec_module(module)
    return module
load('scene_checks',SDK/'indexed-scene/run.py')
handoff=load('range_handoff',SDK/'foundation-ranges/run.py')


class RangeHandoffTests(unittest.TestCase):
    device=dict(vendor=4098,device=29471,api=[1,4,354])

    def profile(self,capacity):
        return [('DEVICE',self.device.copy()),('PUBLIC_SCENE',dict(foundation=1,index_bytes=4,gpu_generated=True)),
                ('GENERATED_SCENE',dict(compute_push_bytes=32,vertex_push_bytes=8)),
                ('DRAW_IDENTITY_FEATURES',dict(shaderDrawParameters=True)),
                ('FRONTIER_FEATURES',dict(multiDrawIndirect=True,drawIndirectCount=True,maxDrawIndirectCount=capacity+7))]

    def log(self,records,drain):
        return ''.join(key+' '+json.dumps(value)+'\n' for key,value in records)+drain

    def reject_mutations(self,records,drain,check):
        check(self.log(records,drain),'')
        for _,row in records:
            for key,value in row.items():
                row[key]=not value if isinstance(value,bool) else [0] if isinstance(value,list) else value-1
                with self.assertRaises(ValueError,msg=key):check(self.log(records,drain),'')
                row[key]=value
        for bad in (self.log(records,'not drained'),self.log(records[1:],drain),self.log(records+[records[1]],drain)):
            with self.assertRaises(ValueError):check(bad,'')
        for error in ('Validation Error: injected','SYNC-HAZARD injected'):
            with self.assertRaises(ValueError):check(self.log(records,drain),error)

    def test_fresh(self):
        for capacity in (1,64,512):
            records=self.profile(capacity)+[('FRONTIER_FRAME',f) for f in range_oracle.frames(capacity)]
            records.append(('FRONTIER_STORAGE',dict(requested_bytes=handoff.requested_bytes(257,193,capacity),arenas=1)))
            self.reject_mutations(records,'Public indexed frontier drained',
                                  lambda out,err:handoff.check_fresh(out,err,self.device,257,193,capacity))

    def test_reuse(self):
        for capacity in (1,64,512):
            for strategy in range_oracle.STRATEGIES:
                for slots in (1,2):
                    for replay in (False,True):
                        for scopes in ((False,True) if strategy=='single' else (False,)):
                            records=self.profile(capacity)+[('RANGE_SCOPE_POLICY',dict(per_draw=scopes))]
                            for f in range(16):
                                generation=f//slots;state=range_oracle.frames(capacity)[generation%8]
                                records.append(('RANGE_FRAME',dict(frame=f,slot=f%slots,generation=generation,phase=state['phase'],active=state['active'])))
                            summary=dict(frames=16,slots=slots,capacity=capacity,strategy=range_oracle.STRATEGIES.index(strategy),replay=int(replay),
                                encodes=slots if replay else 16,peak_unretired=slots,requested_bytes=slots*handoff.requested_bytes(257,193,capacity))
                            records.append(('RANGE_SUMMARY',summary))
                            self.reject_mutations(records,'Public scene reuse drained and checked',
                                lambda out,err:handoff.check_reuse(out,err,self.device,257,193,capacity,strategy,slots,replay,scopes))

    def test_file_corruption(self):
        # Tiny synthetic coverage tests this handoff's exact-byte checks, not the
        # separate analytic rasterizer baseline (covered by test_oracle.py).
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);reference=root/'baseline';reference.mkdir();dest=root/'range';dest.mkdir()
            colors={0:oracle.GREEN,4:oracle.RED,9:oracle.BLACK}
            for mode,color in colors.items():
                for phase in (0,1):
                    image=oracle.GUARD+color+oracle.GUARD*2+struct.pack('<f',1.)+oracle.GUARD
                    (reference/f'mode-{mode}-frame-{phase}.images').write_bytes(image)
            for capacity in (1,64,512):
                for strategy in range_oracle.STRATEGIES:
                    for frame in range_oracle.frames(capacity):
                        active=min(frame['active'],capacity)
                        mode=9 if active==0 else 0 if capacity>1 and active==capacity else 4
                        data=(reference/f'mode-{mode}-frame-{frame["phase"]}.images').read_bytes()
                        (dest/f'frame-{frame["frame"]}.images').write_bytes(range_oracle.identity_image(data,1,1,capacity,strategy))
                        (dest/f'frame-{frame["frame"]}.geometry').write_bytes(range_oracle.mesh_expected(frame,strategy))
                    def check():handoff.check_files(dest,reference,1,1,capacity,strategy)
                    check()
                    for suffix in ('images','geometry'):
                        path=dest/f'frame-0.{suffix}';original=path.read_bytes()
                        offsets=(0,64,192,len(original)-1) if suffix=='images' else (0,64,320,480,len(original)-68,len(original)-1)
                        for offset in offsets:
                            bad=bytearray(original);bad[offset]^=1;path.write_bytes(bad)
                            with self.assertRaises(ValueError,msg=(suffix,offset)):check()
                            path.write_bytes(original)
                        path.write_bytes(original[:-1])
                        with self.assertRaises(ValueError):check()
                        path.write_bytes(original)


if __name__=='__main__':unittest.main()
