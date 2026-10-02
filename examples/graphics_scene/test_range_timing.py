"""Reject mislabeled windows, invalid clocks, missing samples and failed final checks."""
import copy
import json
import itertools
import unittest
import range_timing as t


class TimingTests(unittest.TestCase):
    def test_metrics_and_rejections(self):
        device={'vendor':4098}
        result=dict(width=257,height=193,frames=16,warmups=100,slots=2,capacity=512,strategy=2,replay=1,peak_unretired=2,encodes=2,
                    setup_ms=10.,wall_ms=8.)
        samples=[dict(index=i,record_submit_ms=.1,wait_ms=.2,retirement_ms=.7) for i in range(16)]
        marker='Range timing final-slot bytes checked and all work drained'
        def logs():
            return 'RANGE_SCOPE_POLICY {"per_draw":false}\nDEVICE '+json.dumps(device)+'\nRANGE_TIMING '+json.dumps(result)+'\n'+''.join('SAMPLE '+json.dumps(s)+'\n' for s in samples)+marker
        def parse(stdout=None,stderr=''):
            return t.parse(logs() if stdout is None else stdout,stderr,device,512,'count',2,True,16)
        parsed=parse();self.assertEqual(parsed['wall_ms_per_frame'],.5)
        self.assertEqual(parsed['statistics']['record_submit_ms']['median'],.1)
        for key in ('width','height','frames','warmups','slots','capacity','strategy','replay','peak_unretired','encodes'):
            old=result[key];result[key]+=1
            with self.assertRaises(ValueError):parse()
            result[key]=old
        for target,key,value in ((result,'wall_ms',float('nan')),(result,'wall_ms',-1),(result,'wall_ms',1),
            (result,'setup_ms',float('inf')),(samples[3],'index',4),(samples[3],'wait_ms',-.1),
            (samples[3],'record_submit_ms',float('nan')),(samples[3],'retirement_ms',.01)):
            old=copy.deepcopy(target[key]);target[key]=value
            with self.assertRaises(ValueError):parse()
            target[key]=old
        with self.assertRaises(ValueError):parse(stderr='Validation Error: injected')
        with self.assertRaises(ValueError):parse(logs().replace(marker,''))
        with self.assertRaises(ValueError):parse(logs().replace('"per_draw":false','"per_draw":true'))
        result.update(width=1280,height=720,strategy=0)
        scoped=logs().replace('"per_draw":false','"per_draw":true')
        t.parse(scoped,'',device,512,'single',2,True,16,1280,720,True)
        result.update(width=257,height=193,strategy=2)
        samples.pop()
        with self.assertRaises(ValueError):parse()

    def test_correctness_matrix(self):
        for scale,scopes in itertools.product((False,True),repeat=2):
            extents=[(257,193)]+([(1280,720)] if scale else [])
            runs=[dict(extent=list(extent),backend=backend,summary=dict(capacity=n,strategy=s,slots=slots,replay=replay,
                       frames=16 if extent==(1280,720) else 1000))
                  for extent,n,s,slots,replay,backend in itertools.product(extents,(1,64,512),(0,) if scopes else (0,1,2),(1,2),(0,1),('native','public'))]
            correct=dict(complete=True,scale=scale,per_draw_scopes=scopes,runs=runs)
            self.assertEqual(t.correctness_matrix(correct,scale,scopes,False),extents)
            for wrong in (dict(correct,runs=runs[:-1]),dict(correct,runs=runs+[runs[0]]),dict(correct,scale=not scale),
                          dict(correct,per_draw_scopes=not scopes),dict(correct,software=True),dict(correct,complete=False)):
                with self.assertRaises(ValueError):t.correctness_matrix(wrong,scale,scopes,False)
            runs[0]['summary']['frames']=16
            with self.assertRaises(ValueError):t.correctness_matrix(correct,scale,scopes,False)
            t.correctness_matrix(correct,scale,scopes,True)


if __name__=='__main__':unittest.main()
