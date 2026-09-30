import math
import struct
import unittest
import oracle


class OracleTests(unittest.TestCase):
    def test_occlusion_modes(self):
        wants=[(oracle.GREEN,.25),(oracle.GREEN,.25),(oracle.RED,1.),(oracle.RED,1.),
               (oracle.RED,.75),(oracle.BLACK,0.),(oracle.GREEN,.25),(oracle.BLACK,.25),
               (oracle.RED,.75),(oracle.BLACK,1.)]
        for mode,want in enumerate(wants):
            self.assertEqual(oracle.expected(0,0,0,mode),want)
            self.assertEqual(oracle.expected(.9,.9,0,mode),(oracle.BLACK,0. if mode==5 else 1.))

    def test_moving_rectangle(self):
        self.assertEqual(oracle.expected(-.3,0,0,0),(oracle.GREEN,.25))
        self.assertEqual(oracle.expected(-.3,0,1,0),(oracle.RED,.75))

    def test_geometry(self):
        data=oracle.geometry(0,False)
        self.assertEqual(len(data),584)
        self.assertEqual(struct.unpack('<IIIiI',data[480:500]),(6,1,2,-1,0))
        self.assertEqual(struct.unpack('<IIIiI',data[500:520]),(6,1,2,3,0))
        self.assertEqual(struct.unpack('<I',oracle.geometry(0,True)[480:484]),(0,))

    def test_corruption_rejects(self):
        w,h=257,193
        images=bytearray(oracle.GUARD+oracle.BLACK*(w*h)+oracle.GUARD*2+struct.pack('<f',1.)*(w*h)+oracle.GUARD)
        mesh=oracle.geometry(0,True)
        self.assertGreater(oracle.check(images,mesh,w,h,0,9)['checked_pixels'],w*h*.9)
        for offset in (0,64+4*(h//2*w+w//2),len(images)-1):
            bad=images.copy();bad[offset]^=1
            with self.assertRaises(ValueError): oracle.check(bad,mesh,w,h,0,9)
        for value in (0.,math.nan,math.inf):
            bad=images.copy(); offset=192+w*h*4+4*(h//2*w+w//2)
            bad[offset:offset+4]=struct.pack('<f',value)
            with self.assertRaises(ValueError): oracle.check(bad,mesh,w,h,0,9)
        with self.assertRaises(ValueError): oracle.check(images,mesh[:-1],w,h,0,9)
        with self.assertRaises(ValueError): oracle.check(images[:-1],mesh,w,h,0,9)

    def test_index_widths(self):
        for phase in (0,1):
            for empty in (False,True):
                narrow=oracle.geometry(phase,empty,2);wide=oracle.geometry(phase,empty,4)
                self.assertEqual(len(narrow),584)
                self.assertEqual(narrow[:320],wide[:320])
                self.assertEqual(narrow[416:],wide[416:])
                self.assertEqual(struct.unpack('<8H',narrow[320:336]),(65535,65535,1,2,3,1,3,4))
                self.assertEqual(narrow[336:416],bytes([0xa5])*80)
        w,h=257,193
        images=oracle.GUARD+oracle.BLACK*(w*h)+oracle.GUARD*2+struct.pack('<f',1.)*(w*h)+oracle.GUARD
        mesh=oracle.geometry(0,True,2)
        oracle.check(images,mesh,w,h,0,9,2)
        with self.assertRaises(ValueError): oracle.check(images,mesh,w,h,0,9,4)
        damaged=bytearray(mesh);damaged[336]^=1
        with self.assertRaises(ValueError): oracle.check(images,damaged,w,h,0,9,2)
        with self.assertRaises(ValueError): oracle.geometry(0,False,1)


if __name__=='__main__': unittest.main()
