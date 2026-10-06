import itertools
import json
import unittest
import argument_timing as t


class ArgumentTimingTests(unittest.TestCase):
    def test_complete_matrix(self):
        rows=[dict(bytes=b,capacity=c,slots=s,path=p,summary=dict(frames=64,replay=0,strategy=0,capacity=c,slots=s))
              for b,c,s,p in itertools.product((8,64,256),(1,64,512),(1,2),t.a.PATHS)]
        report=dict(complete=True,build_only=False,sizes=[8,64,256],frames=64,runs=rows)
        self.assertEqual(t.matrix(report),[8,64,256])
        for bad in (rows[:-1],rows+[rows[0]],rows[:-1]+[dict(rows[-1],path='unknown')]):
            with self.assertRaises(ValueError):t.matrix(dict(report,runs=bad))
        with self.assertRaises(ValueError):t.matrix(dict(report,build_only=True))

    def test_host_intervals_and_diagnostic_rejection(self):
        device=dict(vendor=1);frames=16
        result=dict(width=257,height=193,frames=frames,warmups=100,slots=2,capacity=512,strategy=0,replay=0,
                    peak_unretired=2,encodes=frames,setup_ms=1.,wall_ms=32.)
        whole=[dict(index=i,record_submit_ms=.2,wait_ms=.8,retirement_ms=2.) for i in range(frames)]
        parts=[dict(index=i,record_ms=.1,submit_ms=.05) for i in range(frames)]
        policy=dict(bytes=256,reverse_alternating=True,resupply=True)
        def log():
            return '\n'.join(['DEVICE '+json.dumps(device),'RANGE_SCOPE_POLICY '+json.dumps(dict(per_draw=False)),
                'ARGUMENT_POLICY '+json.dumps(policy),'RANGE_TIMING '+json.dumps(result)]+
                ['SAMPLE '+json.dumps(s) for s in whole]+['ARGUMENT_SAMPLE '+json.dumps(s) for s in parts]+
                ['Range timing final-slot bytes checked and all work drained'])
        parsed=t.parse(log(),'',device,'public',256,512,2,frames)
        self.assertEqual(parsed['statistics']['record_ms']['median'],.1)
        for key,value in (('record_ms',float('nan')),('submit_ms',-.1),('record_ms',.3),('index',3)):
            old=parts[0][key];parts[0][key]=value
            with self.assertRaises(ValueError):t.parse(log(),'',device,'public',256,512,2,frames)
            parts[0][key]=old
        for extra in ('\nARGUMENT_INPUT {}','\nARGUMENT_SAMPLE {}'):
            with self.assertRaises((ValueError,KeyError)):t.parse(log()+extra,'',device,'public',256,512,2,frames)
        with self.assertRaises(ValueError):t.parse(log(),'ARGUMENT_COMMANDS {}',device,'public',256,512,2,frames)


if __name__=='__main__':unittest.main()
