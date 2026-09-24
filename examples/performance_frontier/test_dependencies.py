#!/usr/bin/env python3
"""Exhaustive same-stage graph and dependency evidence rejection tests."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("dependencies", Path(__file__).with_name("dependencies.py"))
d = importlib.util.module_from_spec(spec); spec.loader.exec_module(d)


class DependencyTests(unittest.TestCase):
    def test_graph(self):
        self.assertEqual(d.graph_check(), {"a-bc": ["ab","ac"], "ab-c": ["ac","bc"],
                                          "ba-c": ["ac","bc"], "a-cb": ["ab","ac"]})
        durations = dict(a=1,b=2,c=1)
        def span(edges):
            def end(n): return durations[n] + max((end(a) for a,b in edges if b == n), default=0)
            return max(end(n) for n in durations)
        self.assertEqual(span({("a","c")}),2)
        for order in d.ORDERS: self.assertEqual(span(d.barrier_edges(order)),3)

    def fixture(self):
        return dict(strategy="split", order="a-bc", kib=64, frames=1, warmups=100,
                    validation=False, setup_ms=1.0, wall_ms=1.0, event_count=1)

    def text(self, result=None, frame=None):
        return "DEVICE {}\nDEPENDENCY_RESULT "+json.dumps(result or self.fixture())+"\nFRAME "+json.dumps(
            frame or dict(index=0,submit_ms=.1,wait_ms=.2,latency_ms=.3))+"\nP4 exact X/Y outputs and guards PASS; all executions drained\n"

    def test_valid_and_summary(self):
        r, _, frames=d.parse(self.text(),(64,"split","a-bc"),1,False)
        self.assertEqual(r,self.fixture()); self.assertEqual(d.summarize(frames)["wait_ms"]["median"],.2)

    def test_policy_rejection(self):
        for key,value in [("event_count",0),("order","ab-c"),("kib",4096),("frames",2),("warmups",0),("validation",True)]:
            r=copy.deepcopy(self.fixture()); r[key]=value
            with self.assertRaises(RuntimeError): d.parse(self.text(r),(64,"split","a-bc"),1,False)

    def test_sample_rejection(self):
        for frame in [dict(index=1,submit_ms=.1,wait_ms=.2,latency_ms=.3),
                      dict(index=0,submit_ms=.1,wait_ms=.2,latency_ms=.4),
                      dict(index=0,submit_ms=float("nan"),wait_ms=.2,latency_ms=.3)]:
            with self.assertRaises(RuntimeError): d.parse(self.text(frame=frame),(64,"split","a-bc"),1,False)

    def test_missing_oracle(self):
        with self.assertRaises(RuntimeError): d.parse(self.text().replace("guards PASS","guards FAIL"),(64,"split","a-bc"),1,False)

    def test_incomplete_export(self):
        with tempfile.TemporaryDirectory(prefix="dependency-export-test-") as folder:
            root=Path(folder)
            for report in [dict(schema=1,complete=False),dict(schema=2,complete=True),
                           dict(schema=1,complete=True,graph=d.graph_check(),software=True,validation=[])]:
                source=root/"report.json"; source.write_text(json.dumps(report))
                with self.assertRaises(RuntimeError): d.export(source,root/"out")
                self.assertFalse((root/"out").exists())

    def test_memory_rejection(self):
        stderr='\n'.join(['ALLOCATE {"id":1,"bytes":65664,"type":5}',
            'ALLOCATE {"id":2,"bytes":65664,"type":5}','FREE {"id":1}','FREE {"id":2}',
            'MEMORY_SUMMARY {"allocations":2,"frees":2,"peak_bytes":131328,"peak_count":2,"live_bytes":0,"live_count":0}'])
        result=dict(kib=64,strategy="ogpu"); signatures={}
        d.check_memory(dict(result=result),"",stderr,signatures)
        for broken in [stderr.replace('"type":5','"type":6'),stderr.replace('65664','65600'),
                       stderr.replace('"frees":2','"frees":1')]:
            with self.assertRaises((RuntimeError,ValueError)): d.check_memory(dict(result=result),"",broken,signatures)


if __name__ == "__main__": unittest.main()
