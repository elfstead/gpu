import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("host_access", Path(__file__).with_name("host_access.py"))
h = importlib.util.module_from_spec(spec); spec.loader.exec_module(h)


class HostEvidence(unittest.TestCase):
    def output(self, policy="mapped"):
        n = 64 * 1024 // 4
        sums = [sum((i * 17 + seed) * 3 % (2**32) + 7 for i in range(n)) for seed in (1, 0x80001234)]
        result = dict(policy=policy, slots=2, kib=64, frames=4, warmups=100, validation=False,
                      staging_bytes=0 if policy in ("mapped", "shared", "ogpu-mapped", "ogpu-shared") else 2 * (65536+128),
                      stride=65536+128, requested_bytes=2*(65536+128), checksum=2*sum(sums), setup_ms=1., wall_ms=4.)
        views = []
        if policy in ("ogpu-mapped", "ogpu-shared"):
            views = ['HOST_VIEW '+json.dumps(dict(size=(131328 if policy=="ogpu-shared" else 65664), alignment=64, granularity=1, coherent=1))] * (1 if policy=="ogpu-shared" else 2)
        return '\n'.join(['DEVICE {"vendor":1,"device":2,"api":[1,4,0]}', 'HOST_RESULT '+json.dumps(result), *views,
            *('HOST_FRAME '+json.dumps(dict(index=i, **{k:.1 if k != "latency_ms" else .7 for k in h.FIELDS})) for i in range(4)),
            'HOST_ACCESS full outputs/guards/checksums PASS; all slots drained'])

    def parse(self, output, policy="mapped"):
        return h.parse(output, 2, 64, policy, 4, False)

    def test_matrix_and_samples(self):
        self.assertEqual(len(h.matrix()), 14)
        self.assertEqual(len(set(h.matrix())), 14)
        self.assertNotIn((1,64,"shared"), h.matrix())
        self.assertEqual(len(h.matrix(2)),20)
        with self.assertRaises(RuntimeError): h.matrix(3)
        for p in h.policies(2):
            _,_,rows = self.parse(self.output(p),p)
            self.assertAlmostEqual(h.summarize(rows)["cpu_access_ms"]["median"], .4)

    def test_corruption(self):
        for before,after in [('"staging_bytes": 0','"staging_bytes": 100'), ('"stride": 65664','"stride": 65536'),
                             ('"produce_ms": 0.1','"produce_ms": NaN'), ('"latency_ms": 0.7','"latency_ms": 0.1'),
                             ('"frames": 4','"frames": 3'), ('"index": 1','"index": 0')]:
            with self.assertRaises(RuntimeError): self.parse(self.output().replace(before,after))
        text = self.output(); rows = text.splitlines()
        r = json.loads(rows[1].removeprefix('HOST_RESULT ')); r['checksum'] += 1
        rows[1] = 'HOST_RESULT '+json.dumps(r)
        with self.assertRaises(RuntimeError): self.parse('\n'.join(rows))
        with self.assertRaises(RuntimeError): self.parse('\n'.join(x for x in text.splitlines() if not x.startswith('HOST_FRAME ')))

    def test_allocation_isolation(self):
        signatures = {}; types = {}; r = dict(slots=2,kib=64,policy="mapped")
        h.check_signatures(signatures,types,r,[(65664,1),(65664,1)])
        for bad in ([(65664,2),(65664,2)],[(65680,1),(65680,1)]):
            with self.assertRaises(RuntimeError): h.check_signatures(signatures,types,r,bad)
        h.check_signatures(signatures,types,dict(r,policy="shared"),[(131328,1)])
        with self.assertRaises(RuntimeError): h.check_signatures(signatures,types,dict(r,policy="shared"),[(131328,2)])

    def test_incomplete_export(self):
        with tempfile.TemporaryDirectory(prefix="host-export-test-") as folder:
            root = Path(folder)
            for report in (dict(schema=1,complete=False), dict(schema=3,complete=True),
                           dict(schema=2,complete=True,software=True,validation=[]),
                           dict(schema=1,complete=True,software=True,validation=[])):
                source = root/'report.json'; source.write_text(json.dumps(report))
                with self.assertRaises(RuntimeError): h.export(source,root/'out')
                self.assertFalse((root/'out').exists())

    def test_gate_size_and_marker(self):
        identity = dict(vendor=1,device=2,api=[1,4,0])
        stdout = '\n'.join(('DEVICE '+json.dumps(identity),
            'HOST_MEMORY {"atom":64,"types":[0,6]}',
            'NATIVE_BUFFER {"host":true,"requested":131328,"allocated":131328,"type":1,"flags":6}',
            'HOST_GATE disjoint CPU read/write/flush while other range pending PASS',
            'HOST_ACCESS full outputs/guards/checksums PASS; all slots drained'))
        stderr = '\n'.join(('ALLOCATE {"id":1,"bytes":131328,"type":1}', 'FREE {"id":1}',
            'MEMORY_SUMMARY {"allocations":1,"frees":1,"peak_bytes":131328,"peak_count":1,"live_bytes":0,"live_count":0}'))
        h.gate_evidence(stdout,stderr,"shared",64,identity)
        with self.assertRaises(RuntimeError): h.gate_evidence(stdout,stderr,"shared",4096,identity)
        with self.assertRaises(RuntimeError): h.gate_evidence(stdout.replace('pending PASS','pending FAIL'),stderr,"shared",64,identity)


if __name__ == '__main__': unittest.main()
