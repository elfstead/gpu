#!/usr/bin/env python3
"""Acceptance checker must reject stale, malformed and numerically wrong output."""
import struct
from pathlib import Path
import sys
import tempfile
import unittest
sys.dont_write_bytecode = True
import run


class RunnerTests(unittest.TestCase):
    def test_float_success(self):
        error, ratio = run.compare_float(struct.pack("<2f", 0.0, 0.5), struct.pack("<2d", 0.0, 0.5))
        self.assertEqual((error, ratio), (0.0, 0.0))

    def test_float_rejections(self):
        for actual in (b"", struct.pack("<2f", 0.5, 0.5), struct.pack("<f", float("nan")),
                       struct.pack("<f", float("inf")), struct.pack("<f", 0.501)):
            with self.assertRaises(ValueError):
                run.compare_float(actual, struct.pack("<d", 0.5))
        with self.assertRaises(ValueError):
            run.compare_float(struct.pack("<f", 0.5), struct.pack("<d", float("nan")))

    def test_pixels_success(self):
        self.assertEqual(run.compare_pixels(bytes([5, 7, 9, 255]), bytes([6, 7, 8, 255])), 1)

    def test_pixels_rejections(self):
        for actual in (b"", b"123", bytes([2, 7, 8, 255]), bytes([6, 7, 8, 254])):
            with self.assertRaises(ValueError):
                run.compare_pixels(actual, bytes([6, 7, 8, 255]))

    def test_streaming_comparisons(self):
        with tempfile.TemporaryDirectory() as directory:
            a, b = Path(directory) / "gpu", Path(directory) / "cpu"
            count = 65537  # Beyond the first comparison chunk.
            a.write_bytes(struct.pack("<f", 0.5) * count)
            b.write_bytes(struct.pack("<d", 0.5) * count)
            self.assertEqual(run.compare_files(a, b, floating=True), (0, 0))
            with a.open("r+b") as output:
                output.seek(65536 * 4)
                output.write(struct.pack("<f", float("nan")))
            with self.assertRaisesRegex(ValueError, "byte 262144"):
                run.compare_files(a, b, floating=True)
            a.write_bytes(bytes([5, 7, 9, 255]) * count)
            b.write_bytes(a.read_bytes())
            self.assertTrue(run.same_file(a, b))
            self.assertEqual(run.compare_files(a, b), 0)
            with b.open("r+b") as output:
                output.seek(-1, 2)
                output.write(b"\x00")
            self.assertFalse(run.same_file(a, b))
            with self.assertRaisesRegex(ValueError, "byte 262144"):
                run.compare_files(a, b)
            b.write_bytes(b.read_bytes()[:-1])
            with self.assertRaisesRegex(ValueError, "extent"):
                run.compare_files(a, b)


if __name__ == "__main__":
    unittest.main()
