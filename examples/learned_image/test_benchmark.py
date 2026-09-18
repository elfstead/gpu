import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
sys.dont_write_bytecode = True
import benchmark as bench
import export_measurement


class MeasurementTests(unittest.TestCase):
    def test_export_rejects_incomplete_report_without_writing(self):
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory) / "report.json"
            destination = Path(directory) / "receipt"
            for value in ({}, {"schema": 1, "complete": False}, {"schema": 1, "complete": True, "runs": []}):
                report.write_text(json.dumps(value))
                with self.assertRaises(ValueError):
                    export_measurement.export(report, destination)
                self.assertFalse(destination.exists())

    def fixture(self):
        metadata = dict(mode="resident", validation=False, warmups=10, frames=30, setup_ms=1.0)
        samples = [dict(frame=i, input=i % 2, **{f: .1 for f in bench.FIELDS}) for i in range(10, 40)]
        for s in samples:
            s["total_ms"], s["device_ms"] = .7, .3
        return metadata, samples

    def encoded(self, m, s):
        return "MEASUREMENT " + json.dumps(m) + "\n" + "\n".join("SAMPLE " + json.dumps(row) for row in s)

    def test_samples_and_statistics(self):
        m, s = self.fixture()
        self.assertEqual(bench.parse_samples(self.encoded(m, s), "resident"), (m, s))
        self.assertEqual(bench.summarize(s)["total_ms"]["median"], .7)
        for row in s:
            row["device_ms"] = None
        bench.parse_samples(self.encoded(m, s), "resident")
        self.assertIsNone(bench.summarize(s)["device_ms"])

    def test_reject_sample_corruption(self):
        m, s = self.fixture()
        mutations = [lambda rows: rows.pop(), lambda rows: rows.append(rows[-1]),
                     lambda rows: rows[0].update(frame=11), lambda rows: rows[1].update(input=0),
                     lambda rows: rows[0].update(total_ms=0), lambda rows: rows[0].update(total_ms=1),
                     lambda rows: rows[0].update(wait_ms=-1), lambda rows: rows[0].update(wait_ms=True),
                     lambda rows: rows[0].update(wait_ms=float("nan")),
                     lambda rows: rows[0].update(device_ms=float("inf")),
                     lambda rows: rows[0].update(device_ms=0), lambda rows: rows[0].update(device_ms=None)]
        for mutate in mutations:
            altered = copy.deepcopy(s)
            mutate(altered)
            with self.assertRaises(ValueError):
                bench.parse_samples(self.encoded(m, altered), "resident")
        for field, value in (("mode", "end-to-end"), ("validation", True), ("warmups", 9), ("frames", 29)):
            with self.assertRaises(ValueError):
                bench.parse_samples(self.encoded(dict(m, **{field: value}), s), "resident")
        with self.assertRaises(ValueError):
            bench.parse_samples(self.encoded(m, s) + "\nMEASUREMENT " + json.dumps(m), "resident")

    def memory_trace(self):
        events = [("ALLOCATE", dict(id=1, bytes=64, type=0)), ("ALLOCATE", dict(id=2, bytes=32, type=1)),
                  ("FREE", dict(id=1)), ("ALLOCATE", dict(id=1, bytes=128, type=0)),
                  ("FREE", dict(id=2)), ("FREE", dict(id=1))]
        summary = dict(allocations=3, frees=3, peak_bytes=160, peak_count=2, live_bytes=0, live_count=0)
        text = "\n".join(name + " " + json.dumps(row) for name, row in events)
        return text, summary

    def test_memory_trace(self):
        text, summary = self.memory_trace()
        self.assertEqual(bench.parse_memory(text + "\nMEMORY_SUMMARY " + json.dumps(summary)), summary)
        for altered in (text + '\nFREE {"id":1}',
                        text.replace('"bytes": 64', '"bytes": 0'),
                        text.replace('"id": 2', '"id": 1'),
                        text + '\nALLOCATE {"id":3,"bytes":64,"type":1}'):
            with self.assertRaises(ValueError):
                bench.parse_memory(altered + "\nMEMORY_SUMMARY " + json.dumps(summary))
        with self.assertRaises(ValueError):
            bench.parse_memory(text + "\nMEMORY_SUMMARY " + json.dumps(dict(summary, peak_bytes=1)))
        with self.assertRaises(ValueError):
            bench.parse_memory(text)

    def test_metadata_accounting(self):
        # 1x1 -> 1x1: five guarded allocations, plus resident input B.
        m = dict(mode="resident", validation=False, warmups=10, frames=30,
                 device_buffers=1184, host_buffers=632, cpu_payload=1584,
                 image_logical=4, upload_bytes=0, readback_bytes=0)
        bench.check_metadata(m, (1, 1, 1, 1), "resident", False)
        for field in ("device_buffers", "host_buffers", "cpu_payload", "upload_bytes", "readback_bytes"):
            with self.assertRaises(ValueError):
                bench.check_metadata(dict(m, **{field: m[field] + 1}), (1, 1, 1, 1), "resident", False)


if __name__ == "__main__":
    unittest.main()
