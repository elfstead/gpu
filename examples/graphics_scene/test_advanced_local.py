import struct
import tempfile
import unittest
from pathlib import Path
from advanced_checks import check_local_reads


class LocalReadOracle(unittest.TestCase):
    def make(self,folder):
        color=bytes([10,20,30,255,40,50,60,255])
        images=b'\xa5'*64+color+b'\xa5'*128+struct.pack('<2f',.25,.75)+b'\xa5'*64
        local=b'\xa5'*64+bytes([30,10,64,255,60,40,191,255])+b'\xa5'*64
        for mode in range(6):
            for frame in range(3):
                stem=folder/f'mode-{mode}-frame-{frame}'
                stem.with_suffix('.images').write_bytes(images)
                stem.with_suffix('.local').write_bytes(local)

    def test_color_depth_swizzle_and_quantization(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp);self.make(p)
            self.assertEqual(check_local_reads(p,2,1),dict(pixels=36,max_byte_delta=0,depth_read=True))
            path=p/'mode-0-frame-0.local';data=bytearray(path.read_bytes());data[66]+=4;path.write_bytes(data)
            with self.assertRaisesRegex(ValueError,'color/depth'):check_local_reads(p,2,1)

    def test_guard_is_part_of_acceptance(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp);self.make(p)
            path=p/'mode-0-frame-0.local';data=bytearray(path.read_bytes());data[-1]=0;path.write_bytes(data)
            with self.assertRaisesRegex(ValueError,'guards'):check_local_reads(p,2,1)


if __name__=='__main__':unittest.main()
