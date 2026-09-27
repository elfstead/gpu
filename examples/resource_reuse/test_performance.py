import importlib.util
import json
from pathlib import Path
import unittest
spec = importlib.util.spec_from_file_location("reuse_performance",Path(__file__).with_name("performance.py"))
p = importlib.util.module_from_spec(spec); spec.loader.exec_module(p)
spec2 = importlib.util.spec_from_file_location("reuse_parser_tests",Path(__file__).with_name("test_runner.py"))
t = importlib.util.module_from_spec(spec2); spec2.loader.exec_module(t)


class PerformanceTests(unittest.TestCase):
    def fixture(self,validation):
        data = t.EvidenceTests().output()
        data += '\nREUSE_CPU_STORAGE '+json.dumps(dict(stream_stack_bytes=1024,sample_bytes=5600,fixture_bytes=512,
                runtime_command_bytes=None,driver_command_bytes=None))
        if not validation:
            data = data.replace('"warmups": 0','"warmups": 100').replace('"validation": true','"validation": false')
            for i in range(12):
                data += '\nFRAME '+json.dumps(dict(index=i,upload_ms=1.,record_ms=1.,submit_ms=1.,wait_ms=1.,read_ms=1.,latency_ms=6.))
        return data

    def parse(self,validation=True,traced=False,change=None):
        stdout = self.fixture(validation); stderr = t.EvidenceTests().memory() if traced else ''
        if change: stdout,stderr = change(stdout,stderr)
        return p.parse(stdout,stderr,"arena","ogpu",1,(65,47,131,95),12,validation,traced)

    def test_valid(self):
        self.assertEqual(self.parse(True,True)[0]["memory"]["allocations"],5)
        self.assertEqual(len(self.parse(False)[1]),12)

    def test_reject_trace_in_timing(self):
        with self.assertRaises(RuntimeError): self.parse(False,change=lambda o,e:(o,'ALLOCATE {}'))

    def test_reject_bad_storage_and_missing_gate(self):
        for old,new in [('"sample_bytes": 5600','"sample_bytes": 0'),('"runtime_command_bytes": null','"runtime_command_bytes": 0'),
                        ('"buffer_allocations":4','"buffer_allocations":9'),('Reuse HOST padding','Missing HOST padding')]:
            with self.subTest(old=old), self.assertRaises(RuntimeError):
                self.parse(change=lambda o,e:(o.replace(old,new),e))

    def test_order(self):
        for slots in (1,2,3):
            for extent in (0,1):
                orders = [p.order(r,extent,slots) for r in range(p.ROUNDS)]
                self.assertEqual(len(set(orders)),p.ROUNDS)
                for order in orders: self.assertEqual(set(order),set(p.CONTROLS))

    def test_process_statistics_not_pooled(self):
        row,samples = self.parse(False)
        rows = [dict(row,round=r,result=dict(row["result"],wall_ms=(r+1)*12)) for r in range(p.ROUNDS)]
        self.assertEqual(p.comparisons(rows)[0]["wall_ms_per_frame"],2.5)
        with self.assertRaises(RuntimeError): p.comparisons(rows[:-1])
        rows[-1]["round"] = 0
        with self.assertRaises(RuntimeError): p.comparisons(rows)


if __name__ == "__main__": unittest.main()
