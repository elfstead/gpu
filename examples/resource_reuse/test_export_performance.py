#!/usr/bin/env python3
"""Offline negative export tests against an existing complete hardware report; no GPU."""
import copy
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
sys.dont_write_bytecode = True
spec=importlib.util.spec_from_file_location("export_reuse",Path(__file__).with_name("export_performance.py"))
exporter=importlib.util.module_from_spec(spec); spec.loader.exec_module(exporter)


class ExportTests(unittest.TestCase):
    source = None

    @classmethod
    def setUpClass(cls):
        cls.report=json.loads(cls.source.read_text())
        if not cls.report.get("complete") or cls.report.get("software") or cls.report.get("dirty"):
            raise ValueError("supply a complete, clean hardware report with timing")

    def reject(self,mutate,message):
        report=copy.deepcopy(self.report); mutate(report)
        # Keep raw-log paths relative to their original directory. Only the
        # temporary report and its new export destination are test-owned.
        with tempfile.NamedTemporaryFile(mode="w",suffix=".json",prefix="export-negative-",dir=self.source.parent) as fixture:
            json.dump(report,fixture); fixture.flush()
            with tempfile.TemporaryDirectory(prefix="reuse-export-test-") as directory:
                destination=Path(directory)/"export"
                with self.assertRaisesRegex(RuntimeError,message): exporter.export(Path(fixture.name),destination)
                self.assertFalse(destination.exists(),"rejected report published evidence")

    def test_incomplete(self): self.reject(lambda r:r.update(complete=False),"incomplete/dirty")
    def test_dirty(self): self.reject(lambda r:r.update(dirty=True),"incomplete/dirty")
    def test_duplicate_matrix(self):
        self.reject(lambda r:r["validation"].__setitem__(0,r["validation"][1]),"duplicate validation")
    def test_sample_hash(self):
        self.reject(lambda r:r["timing"][0].update(samples_sha256="0"*64),"sample hash mismatch")
    def test_statistics(self):
        self.reject(lambda r:r["timing"][0]["statistics"]["latency_ms"].update(median=-1),"statistics mismatch")
    def test_allocation(self):
        self.reject(lambda r:r["validation"][0]["allocation_signature"][0].__setitem__(0,1),"allocation_signature mismatch")
    def test_layout(self):
        self.reject(lambda r:r["validation"][0]["ranges"][0].update(offset=999),"ranges mismatch")
    def test_source_hash(self):
        self.reject(lambda r:r["sources"].update({"examples/resource_reuse/performance.py":"0"*64}),"source mismatch")
    def test_order(self):
        def mutate(r):
            # Swap within round 1, after every summary group was introduced, so
            # summary ordering remains valid and the explicit process-order gate fires.
            i=len(r["timing"])//exporter.p.ROUNDS
            r["timing"][i],r["timing"][i+1]=r["timing"][i+1],r["timing"][i]
        self.reject(mutate,"process order mismatch")
    def test_timing_environment(self):
        self.reject(lambda r:r["environments"]["timing"].update(OGPU_TRACE_LOADER="instrumented"),"instrumented timing")


if __name__=="__main__":
    if len(sys.argv)!=2: raise SystemExit("usage: test_export_performance.py /original/run/report.json")
    ExportTests.source=Path(sys.argv[1]).resolve()
    unittest.main(argv=[sys.argv[0]])
