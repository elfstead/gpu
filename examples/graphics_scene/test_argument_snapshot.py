import json
import struct
import unittest
import argument_snapshot as a
import oracle


def fixture(size):
    images=[]
    for snapshot in range(4):
        phase=int(snapshot==2);pixels=bytearray();depths=bytearray()
        colors={oracle.BLACK:oracle.BLACK,oracle.RED:a.color(size,0,0),oracle.GREEN:a.color(size,1,phase)}
        for row in range(a.HEIGHT):
            for column in range(a.WIDTH):
                color,depth=oracle.expected(2*(column+.5)/a.WIDTH-1,2*(row+.5)/a.HEIGHT-1,phase,0)
                pixels.extend(colors[color]);depths.extend(struct.pack('<f',depth))
        images.append(oracle.GUARD+pixels+oracle.GUARD*2+depths+oracle.GUARD)
    return b''.join(images)


class ArgumentSnapshots(unittest.TestCase):
    def test_payload_and_size(self):
        self.assertEqual(a.color(8,0,0),bytes((2,0,255,255)))
        for size in (64,256):
            n=(size-8)//4
            identity=2+4*sum(i*i for i in range(1,n+1))
            self.assertEqual(a.color(size,0,0),bytes((identity&255,(identity>>8)&255,255,255)))
            # Every individual word has a nonzero, non-wrapping color influence.
            self.assertTrue(all((4*(i+1))%65536 for i in range(n)))
            self.assertNotEqual(a.color(size,1,0),a.color(size,1,1))
        with self.assertRaises(ValueError):a.color(32,0,0)

    def test_analytic_snapshots(self):
        for size in a.SIZES:
            data=fixture(size)
            self.assertEqual(len(a.check(data,size)),4)
            for bad in (data[:-1],data[:a.IMAGE_BYTES]*4,bytes([0])+data[1:]):
                with self.assertRaises(ValueError):a.check(bad,size)
        good=fixture(64)
        # Preserve A/A/B/A equality while poisoning an interior pixel/depth.
        for offset in (64+4*(50*a.WIDTH+50),192+4*a.WIDTH*a.HEIGHT+4*(50*a.WIDTH+50)):
            bad=bytearray(good)
            for snapshot in range(4):bad[snapshot*a.IMAGE_BYTES+offset]^=255
            with self.assertRaises(ValueError):a.check(bad,64)
        with self.assertRaises(ValueError):a.check(good,256)

    def test_strict_logs(self):
        device=dict(vendor=4098,device=29471,api=[1,4,354]);limit=dict(max_push_data_bytes=256)
        for backend in ('native','public'):
            lines={'DEVICE':device,'ARGUMENT_LIMIT':limit,
                'ARGUMENT_SNAPSHOTS':dict(bytes=64,snapshots=4,draws=8,invalid_retries=0 if backend=='native' else 8)}
            lines.update({'DRAW_IDENTITY_FEATURES':dict(shaderDrawParameters=True)} if backend=='native' else
                         {'PUBLIC_SCENE':dict(abi=19,index_bytes=4,gpu_generated=True)})
            log='\n'.join(k+' '+json.dumps(v) for k,v in lines.items())+'\nArgument snapshot control drained'
            self.assertEqual(a.check_log(log,'',backend,64),(limit,device))
            for bad in (log.replace('control drained','unfinished'),log.replace('"snapshots": 4','"snapshots": 3'),
                        log.replace('"max_push_data_bytes": 256','"max_push_data_bytes": 8'),log+'\nDEVICE '+json.dumps(device)):
                with self.assertRaises(ValueError):a.check_log(bad,'',backend,64)
            for error in ('Validation Error: bad','SYNC-HAZARD bad'):
                with self.assertRaises(ValueError):a.check_log(log,error,backend,64)


if __name__=='__main__':unittest.main()
