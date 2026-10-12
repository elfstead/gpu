import json
import os
import shlex
import subprocess
import tempfile
from pathlib import Path
import unittest
import argument_reuse as a


class ArgumentReuseTests(unittest.TestCase):
    def test_c_root_order_and_accounting(self):
        with tempfile.TemporaryDirectory(prefix='ogpu-argument-test-') as directory:
            for size in a.snapshot.SIZES:
                binary=Path(directory)/str(size)
                subprocess.run([*shlex.split(os.getenv('CC','cc')),'-std=c11','-O2','-Wall','-Wextra','-Werror',
                    f'-DSCENE_RASTER_BYTES={size}',str(a.HERE/'test_argument_reuse.c'),'-o',str(binary)],check=True)
                subprocess.run([str(binary)],check=True,capture_output=True)

    def test_payload_recolor_preserves_noncolor_bytes(self):
        color=bytes((2,0,255,255))+bytes((3,0,255,255))
        data=bytes([0xa5])*64+color+bytes((0,0,0,255))*(257*193-2)+bytes([0xa5])*(257*193*4+192)
        self.assertEqual(a.recolor(data,8),data)
        for size in (64,256):
            actual=a.recolor(data,size)
            self.assertEqual(actual[64:72],a.snapshot.color(size,0,0)+a.snapshot.color(size,1,0))
            self.assertEqual(actual[:64],data[:64]);self.assertEqual(actual[72:],data[72:])
        for bad in (data[:-1],data[:64]+bytes((255,0,0,255))+data[68:]):
            with self.assertRaises(ValueError):a.recolor(bad,8)

    def test_supplied_and_native_bytes_are_distinct(self):
        for path in set(a.PATHS+a.FOUNDATION_PATHS):
            for size in a.snapshot.SIZES:
                for capacity in (1,64,512):
                    frames=16;calls=frames*(capacity if a.resupply(path) else 1)
                    pushes=capacity if path in ('native-resupply','foundation-resupply') else 1
                    policy=dict(bytes=size,reverse_alternating=True,resupply=a.resupply(path))
                    supplied=dict(calls=calls,bytes=calls*size)
                    traces=[dict(phase=0,push_bytes=0,forward=0,backward=0,other=0)]+[
                        dict(phase=i,push_bytes=frames*(32+pushes*size),forward=8*(capacity-1),backward=8*(capacity-1),other=0) for i in (1,2)]
                    def logs():
                        return ('ARGUMENT_POLICY '+json.dumps(policy)+'\nARGUMENT_INPUT '+json.dumps(supplied),
                                '\n'.join('ARGUMENT_COMMANDS '+json.dumps(t) for t in traces))
                    a.check_arguments(*logs(),path,size,capacity,frames)
                    for target,key in [(supplied,'calls'),(supplied,'bytes'),(policy,'bytes'),(policy,'reverse_alternating'),
                                       (policy,'resupply')]+[(t,k) for t in traces for k in ('push_bytes','forward','backward','other')]:
                        original=target[key];target[key]=not original if isinstance(original,bool) else original+1
                        with self.assertRaises(ValueError):a.check_arguments(*logs(),path,size,capacity,frames)
                        target[key]=original

    def test_build_modes(self):
        for path in a.FOUNDATION_PATHS:
            opts=a.flags(['cc'],64,path)
            self.assertNotIn('-DSCENE_ARGUMENT_DIAGNOSTICS',opts)
            self.assertIn('-DSCENE_RANGE_REUSE',opts)
            self.assertEqual('-DNATIVE_DRAW_IDENTITY' in opts,not a.public_path(path))
            self.assertEqual('-DSCENE_ARGUMENT_RESUPPLY' in opts,a.resupply(path))
            self.assertEqual('-DSCENE_ARGUMENT_CURRENT' in opts,path=='public-current')
            self.assertIn('-DSCENE_ARGUMENT_DIAGNOSTICS',a.flags(['cc'],64,path,diagnostic=True))
        for path in ('public','public-current'):
            with self.assertRaisesRegex(ValueError,'retired ABI20'):a.flags(['cc'],64,path)

    def test_foundation_has_no_legacy_current_policy(self):
        for path in ('foundation-once','foundation-resupply'):
            opts=a.flags(['cc'],64,path,diagnostic=True)
            self.assertNotIn('-DSCENE_ARGUMENT_CURRENT',opts)
            self.assertFalse(any('Vulkan-Headers' in p for p in opts))
            self.assertEqual('-DSCENE_ARGUMENT_RESUPPLY' in opts,path=='foundation-resupply')


if __name__=='__main__':unittest.main()
