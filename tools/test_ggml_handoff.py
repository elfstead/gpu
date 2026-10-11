"""The optional full-dataset handoff must never skip either input digest."""
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("handoff",Path(__file__).parent / "sdk/ggml/run.py")
handoff = importlib.util.module_from_spec(spec)
spec.loader.exec_module(handoff)


class DigestTests(unittest.TestCase):
    def test_exact_dataset(self):
        with tempfile.TemporaryDirectory() as directory:
            data = Path(directory)
            files = [data / name for name in ("t10k-images-idx3-ubyte","t10k-labels-idx1-ubyte")]
            for index,path in enumerate(files): path.write_bytes(bytes([index]))
            rows = [handoff.digest(p)+"  "+p.name for p in files]
            handoff.verify_data(data,"\n".join(rows))
            for invalid in ("",rows[0],rows[1],"\n".join([*rows,rows[0]]),"\n".join(rows).replace(rows[0][:64],"0"*64)):
                with self.assertRaises(ValueError): handoff.verify_data(data,invalid)
            files[1].write_bytes(b"changed")
            with self.assertRaises(ValueError): handoff.verify_data(data,"\n".join(rows))


if __name__ == "__main__": unittest.main()
