#!/usr/bin/env python3
"""Acceptance checker must reject stale, malformed and numerically wrong output."""
import struct
import sys
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


if __name__ == "__main__":
    unittest.main()
