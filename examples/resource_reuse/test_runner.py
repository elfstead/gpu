import importlib.util
import json
from pathlib import Path
import unittest
spec = importlib.util.spec_from_file_location("reuse_runner", Path(__file__).with_name("run.py"))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class EvidenceTests(unittest.TestCase):
    def output(self):
        record = dict(policy="ogpu", slots=1, extent=[65,47,131,95], frames=12, warmups=0,
                      validation=True, setup_ms=1., wall_ms=2., max_rgb_delta=1)
        lines = ['DEVICE {"vendor":1,"device":2,"api":[1,4,0]}', 'STREAM '+json.dumps(record),
                 'Streaming final input/weight integrity and all intermediate guards PASS',
                 'Streaming full-frame pixels/readback guards PASS; all slots drained',
                 'Reuse HOST padding and all slot generations retired PASS',
                 'REUSE_MEMORY {"allocation":"arena","buffer_allocations":4,"buffer_requested_bytes":1234}']
        for i in range(7):
            offset = i*64 if i < 5 else 0
            lines.append('REUSE_RANGE '+json.dumps(dict(slot=0,resource=i,backing=0 if i<5 else i-4,
                offset=offset,size=64,end=offset+64,atom=1,host=i>=5)))
        return '\n'.join(lines)

    def memory(self):
        lines = []
        for i in range(5): lines.append('ALLOCATE '+json.dumps(dict(id=i+1,bytes=100,type=0)))
        for i in range(5): lines.append('FREE '+json.dumps(dict(id=i+1)))
        lines.append('MEMORY_SUMMARY '+json.dumps(dict(allocations=5,frees=5,peak_bytes=500,peak_count=5,live_bytes=0,live_count=0)))
        return '\n'.join(lines)

    def parse(self, stdout=None, stderr=None):
        return runner.parse(self.output() if stdout is None else stdout, self.memory() if stderr is None else stderr,
                            "arena", "ogpu", 1, (65,47,131,95), 12)

    def test_valid(self): self.assertEqual(self.parse()["memory"]["peak_bytes"], 500)

    def test_invalid_policy_and_counts(self):
        for old,new in [('"allocation":"arena"','"allocation":"dedicated"'),
                        ('"buffer_allocations":4','"buffer_allocations":3'),
                        ('"frames": 12','"frames": 11'), ('"validation": true','"validation": false')]:
            with self.assertRaises(RuntimeError): self.parse(self.output().replace(old,new))

    def test_missing_guard_marker(self):
        with self.assertRaises(RuntimeError): self.parse(self.output().replace('Reuse HOST padding','Missing HOST padding'))

    def test_bad_ranges(self):
        for old,new in [('"atom": 1','"atom": 0'), ('"size": 64','"size": 65'), ('"offset": 0','"offset": 2'),
                        ('"backing": 0','"backing": 1'), ('"offset": 64','"offset": 0')]:
            with self.assertRaises(RuntimeError): self.parse(self.output().replace(old,new))

    def test_leak(self):
        with self.assertRaises((RuntimeError,ValueError)): self.parse(stderr=self.memory().replace('FREE {"id": 1}', ''))


if __name__ == "__main__": unittest.main()
