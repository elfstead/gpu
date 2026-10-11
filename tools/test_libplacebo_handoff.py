"""Host-only stale/incomplete receipt checks; no GPU or compiler dependencies."""
import copy
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("placebo_handoff",Path(__file__).parent / "sdk/libplacebo/run.py")
handoff = importlib.util.module_from_spec(spec)
spec.loader.exec_module(handoff)


class ReceiptTests(unittest.TestCase):
    def test_exact_build(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            handoff.HERE = root
            build = root / "build"; (build / "upstream/src").mkdir(parents=True)
            sources = (root / "backend.c",root / "compiler.cpp",root / "run.py")
            for path in (*sources,*[build / n for n in handoff.EXECUTABLES],build / "upstream/src/libplacebo.so"):
                path.write_bytes(b"original")
            sdk = dict(revision="test-commit",runtime_sha256="runtime",header_sha256="header")
            record = dict(schema=1,complete=True,build=str(build),upstream_revision="3330a515d62139259c26239014f286e233bd3a5c",
                          sdk=sdk,sources_sha256={p.name:handoff.digest(p) for p in sources},
                          executables_sha256={n:handoff.digest(build / n) for n in handoff.EXECUTABLES},
                          upstream_library_sha256=handoff.digest(build / "upstream/src/libplacebo.so"))
            self.assertEqual(handoff.verify_build(record,sdk),build)
            for field,value in (("schema",2),("complete",False),("upstream_revision","wrong"),
                                ("sources_sha256",{}),("executables_sha256",{})):
                invalid = copy.deepcopy(record); invalid[field] = value
                with self.assertRaises(ValueError): handoff.verify_build(invalid,sdk)
            for field in sdk:
                changed = dict(sdk); changed[field] = "changed"
                with self.assertRaisesRegex(ValueError,"SDK changed"): handoff.verify_build(record,changed)
            for path in (sources[0],build / "consumer",build / "upstream/src/libplacebo.so"):
                path.write_bytes(b"changed")
                with self.assertRaises(ValueError): handoff.verify_build(record,sdk)
                path.write_bytes(b"original")
            (root / "new-source.c").write_bytes(b"new")
            with self.assertRaisesRegex(ValueError,"incomplete consumer source receipt"): handoff.verify_build(record,sdk)


if __name__ == "__main__": unittest.main()
