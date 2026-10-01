"""Reject mislabeled windows, invalid clocks, missing samples and failed final checks."""
import copy
import json
import unittest
import range_timing as t


class TimingTests(unittest.TestCase):
    def test_metrics_and_rejections(self):
        device={'vendor':4098}
        result=dict(frames=16,warmups=100,slots=2,capacity=512,strategy=2,replay=1,peak_unretired=2,encodes=2,
                    setup_ms=10.,wall_ms=8.)
        samples=[dict(index=i,record_submit_ms=.1,wait_ms=.2,retirement_ms=.7) for i in range(16)]
        marker='Range timing final-slot bytes checked and all work drained'
        def logs():
            return 'DEVICE '+json.dumps(device)+'\nRANGE_TIMING '+json.dumps(result)+'\n'+''.join('SAMPLE '+json.dumps(s)+'\n' for s in samples)+marker
        def parse(stdout=None,stderr=''):
            return t.parse(logs() if stdout is None else stdout,stderr,device,512,'count',2,True,16)
        parsed=parse();self.assertEqual(parsed['wall_ms_per_frame'],.5)
        self.assertEqual(parsed['statistics']['record_submit_ms']['median'],.1)
        for key in ('frames','warmups','slots','capacity','strategy','replay','peak_unretired','encodes'):
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
        samples.pop()
        with self.assertRaises(ValueError):parse()


if __name__=='__main__':unittest.main()
