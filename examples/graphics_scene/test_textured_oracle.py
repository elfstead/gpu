import math
import struct
import unittest
import textured_oracle as oracle


class TextureTests(unittest.TestCase):
    def test_upload_and_mip_windows(self):
        for phase in (0,1):
            data=oracle.texture(phase)
            self.assertEqual(len(data),340)
            for mip,offset in enumerate((0,256,320,336)):
                self.assertEqual(data[offset:offset+4],bytes(oracle.texel(mip,0,0,phase)))
            for u,v in ((.01,.01),(.23,.71),(.99,.99)):
                self.assertEqual(oracle.sample(0,1,u,v,phase,False),oracle.sample(1,0,u,v,phase,False))
        self.assertNotEqual(oracle.texture(0),oracle.texture(1))

    def test_filters(self):
        for mip in range(4):
            size=8>>mip
            for y in range(size):
                for x in range(size):
                    uv=((x+.5)/size,(y+.5)/size)
                    self.assertEqual(oracle.sample_level(mip,*uv,0,True),oracle.texel(mip,x,y,0)[:3])
        self.assertEqual(oracle.sample_level(0,0,0,0,True),oracle.texel(0,0,0,0)[:3])
        a=oracle.sample_level(0,.5,.5,0,True);b=oracle.sample_level(1,.5,.5,0,True)
        self.assertEqual(oracle.sample(0,.5,.5,.5,0,True),tuple((x+y)/2 for x,y in zip(a,b)))

    def test_depth_overlay_and_view_equivalence(self):
        width,height=257,193
        for phase in (0,1):
            for row in range(0,height,7):
                for col in range(0,width,7):
                    self.assertEqual(oracle.expected(col,row,width,height,phase,1),oracle.expected(col,row,width,height,phase,2))
        color,depth=oracle.expected(128,96,width,height,0,0)
        self.assertEqual(depth,.25) # overlay does not replace depth
        self.assertEqual(color[3],255)
        self.assertGreater(color[2],127)
        self.assertEqual(oracle.expected(0,0,width,height,0,0),((0.,0.,0.,255.),1.))

    def frame(self,phase,mode):
        width,height=257,193
        color=bytearray();depth=bytearray()
        for row in range(height):
            for col in range(width):
                c,z=oracle.expected(col,row,width,height,phase,mode)
                color.extend(math.floor(x+.5) for x in c);depth.extend(struct.pack('<f',z))
        return bytearray(oracle.GUARD+color+oracle.GUARD*2+depth+oracle.GUARD)

    def test_coverage_and_failure_rejection(self):
        for mode in range(len(oracle.CASES)):
            for phase in (0,1):
                frame=self.frame(phase,mode)
                self.assertGreater(oracle.check(frame,257,193,phase,mode)['checked_pixels'],257*193*.85)
        frame=self.frame(0,0)
        for offset in (0,63,len(frame)-1,64):
            bad=bytearray(frame);bad[offset]^=255
            with self.assertRaises(ValueError):oracle.check(bad,257,193,0,0)
        bad=bytearray(frame);struct.pack_into('<f',bad,192+257*193*4,float('nan'))
        with self.assertRaises(ValueError):oracle.check(bad,257,193,0,0)
        with self.assertRaises(ValueError):oracle.check(frame[:-1],257,193,0,0)
        with self.assertRaises(ValueError):oracle.check(frame,257,193,1,0)


if __name__=='__main__':unittest.main()
