"""Host rejection tests for the independent foundation handoff oracle gates."""
import importlib.util
from pathlib import Path
import struct
import sys
import tempfile
import unittest
sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT / "examples/learned_image"))
spec = importlib.util.spec_from_file_location("handoff",ROOT / "tools/sdk/foundation/run.py")
handoff = importlib.util.module_from_spec(spec)
spec.loader.exec_module(handoff)


class OracleTests(unittest.TestCase):
    def test_float_gates(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "values.f32"
            for values in ((0.,1.),(1e-5,1.+1e-5)):
                path.write_bytes(struct.pack("<2f",*values))
                handoff.check_floats(path,[0.,1.])
            for values in ((0.,), (0.,1.,2.), (float("nan"),1.), (0.,float("inf")), (.001,1.)):
                path.write_bytes(struct.pack(f"<{len(values)}f",*values))
                with self.assertRaises(ValueError): handoff.check_floats(path,[0.,1.])

    def test_pixel_gates(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "pixels.rgba"
            for data in (bytes([10,20,30,255]),bytes([11,19,30,255])):
                path.write_bytes(data); handoff.check_pixels(path,bytes([10,20,30,255]))
            for data in (b"",bytes([10,20,30]),bytes([10,20,30,254]),bytes([12,20,30,255])):
                path.write_bytes(data)
                with self.assertRaises(ValueError): handoff.check_pixels(path,bytes([10,20,30,255]))


if __name__ == "__main__": unittest.main()
