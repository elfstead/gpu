#!/usr/bin/env python3
"""CPU-only negative gates for the installed handoff report parser."""
import copy
import importlib.util
import json
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("handoff", Path(__file__).parent / "sdk/resource-reuse/run.py")
handoff = importlib.util.module_from_spec(spec)
spec.loader.exec_module(handoff)


class ReportTests(unittest.TestCase):
    def setUp(self):
        self.rows = [("DEVICE ", dict(vendor=1, device=2, api=[1, 4, 0])),
                     ("STREAM ", dict(policy="compiled", slots=2, extent=handoff.EXTENT, frames=1000,
                                      warmups=0, validation=True, max_rgb_delta=1, setup_ms=1., wall_ms=2.)),
                     ("REUSE_MEMORY ", dict(allocation="arena", buffer_allocations=4))]
        ends = {}
        for slot in range(2):
            for resource in range(7):
                backing = 0 if resource < 5 else resource-4
                offset = ends.get(backing, 0)
                self.rows.append(("REUSE_RANGE ", dict(slot=slot, resource=resource, backing=backing,
                    offset=offset, size=32, end=offset+64, atom=64, host=resource >= 5)))
                ends[backing] = offset+64

    def check(self, rows=None, gates=handoff.GATES, stderr=""):
        stdout = "\n".join([*(prefix+json.dumps(row) for prefix, row in (self.rows if rows is None else rows)), *gates])
        return handoff.check(stdout, stderr, "arena", "compiled", 2, 1000)

    def test_valid(self):
        self.assertEqual(len(self.check()["ranges"]), 14)

    def test_missing_gate(self):
        for gate in handoff.GATES:
            with self.assertRaises(ValueError): self.check(gates=[g for g in handoff.GATES if g != gate])

    def test_missing_duplicate_record(self):
        for index in range(len(self.rows)):
            with self.assertRaises(ValueError): self.check(self.rows[:index]+self.rows[index+1:])
            with self.assertRaises(ValueError): self.check(self.rows+[self.rows[index]])

    def test_wrong_policy_or_pixels(self):
        for key, value in (("policy", "ogpu"), ("slots", 3), ("frames", 999), ("extent", [1, 1, 1, 1]),
                           ("validation", False), ("warmups", 100), ("max_rgb_delta", 2),
                           ("wall_ms", float("nan")), ("setup_ms", 0)):
            rows = copy.deepcopy(self.rows)
            rows[1][1][key] = value
            with self.assertRaises(ValueError): self.check(rows)

    def test_wrong_ownership_or_alignment(self):
        for key, value in (("backing", 7), ("slot", 3), ("resource", 4), ("host", True),
                           ("atom", 0), ("offset", -1), ("size", 0), ("size", 65), ("end", 63)):
            rows = copy.deepcopy(self.rows)
            rows[3][1][key] = value
            with self.assertRaises(ValueError): self.check(rows)

    def test_overlap(self):
        rows = copy.deepcopy(self.rows)
        rows[4][1]["offset"] = 0
        with self.assertRaises(ValueError): self.check(rows)

    def test_validation_error(self):
        with self.assertRaises(ValueError): self.check(stderr="Validation Error: injected")

    def test_timing_samples(self):
        with self.assertRaises(ValueError): self.check(self.rows+[("FRAME ", {})])

    def test_wrong_allocation(self):
        for key, value in (("allocation", "dedicated"), ("buffer_allocations", 5)):
            rows = copy.deepcopy(self.rows)
            rows[2][1][key] = value
            with self.assertRaises(ValueError): self.check(rows)


if __name__ == "__main__":
    unittest.main()
