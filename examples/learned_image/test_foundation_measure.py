import copy
import json
import unittest
import foundation_measure as measure


class ReplayMeasurementTests(unittest.TestCase):
    def fixture(self):
        m = dict(schema=1,mode="resident",validation=False,warmups=10,frames=30,work_lists=2,
                 setup_ms=1.,record_ms=1.,upload_bytes=0,readback_bytes=0,query_bytes=32,
                 timestamp_bits=36,timestamp_period_ns=1.,device_linear_bytes=1184,
                 host_linear_bytes=532,cpu_payload_bytes=748,image_logical_bytes=4)
        samples = [dict(frame=i,input=i%2,**{f:.1 for f in measure.PHASES},total_ms=.5,device_ms=.2) for i in range(10,40)]
        return m,samples

    def text(self,m,s):
        return "REPLAY "+json.dumps(m)+"\n"+"\n".join("SAMPLE "+json.dumps(row) for row in s)

    def test_valid_and_absent_clock(self):
        m,s = self.fixture()
        self.assertEqual(measure.parse(self.text(m,s),"resident",(1,1,1,1)),(m,s))
        m.update(timestamp_bits=0,timestamp_period_ns=0.,query_bytes=0,host_linear_bytes=500)
        for row in s: row["device_ms"] = None
        measure.parse(self.text(m,s),"resident",(1,1,1,1))

    def test_accounting_and_clock_rejections(self):
        m,s = self.fixture()
        for field,value in (("work_lists",1),("validation",True),("frames",29),("schema",True),
                            ("upload_bytes",1),("readback_bytes",4),("query_bytes",16),("host_linear_bytes",531),
                            ("device_linear_bytes",1),("cpu_payload_bytes",1),("timestamp_bits",65),
                            ("timestamp_bits",True),("timestamp_bits",32),("timestamp_period_ns",float("nan")),("timestamp_period_ns",1e308),
                            ("setup_ms",-1),("record_ms",True)):
            with self.subTest(field=field,value=value),self.assertRaises(ValueError):
                measure.parse(self.text(dict(m,**{field:value}),s),"resident",(1,1,1,1))
        with self.assertRaises(ValueError):
            measure.parse(self.text(m,s)+"\nREPLAY "+json.dumps(m),"resident",(1,1,1,1))

    def test_end_to_end_and_validation_accounting(self):
        m,s = self.fixture()
        m.update(mode="end-to-end",work_lists=1,upload_bytes=132,readback_bytes=4,device_linear_bytes=1052,host_linear_bytes=664)
        measure.parse(self.text(m,s),"end-to-end",(1,1,1,1))
        m.update(validation=True,warmups=0,frames=3,host_linear_bytes=1716)
        measure.metadata(self.text(m,[]),"end-to-end",(1,1,1,1),True)

    def test_sample_rejections(self):
        m,s = self.fixture()
        mutations = [lambda a:a.pop(),lambda a:a.append(a[-1]),lambda a:a[0].update(frame=11),
                     lambda a:a[0].update(input=True),lambda a:a[0].update(total_ms=1),
                     lambda a:a[0].update(wait_ms=-1),lambda a:a[0].update(device_ms=0),
                     lambda a:a[0].update(device_ms=None),lambda a:a[0].update(device_ms=float("inf")),
                     lambda a:a[0].update(device_ms=1e10)]
        for mutate in mutations:
            altered = copy.deepcopy(s); mutate(altered)
            with self.assertRaises(ValueError): measure.parse(self.text(m,altered),"resident",(1,1,1,1))


if __name__ == "__main__": unittest.main()
