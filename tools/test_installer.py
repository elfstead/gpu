"""Safety checks: invalid/existing destinations must fail before invoking Cargo."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import sys
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("installer", Path(__file__).with_name("install.py"))
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)


class InstallerTests(unittest.TestCase):
    def reject(self, prefix):
        with patch.object(installer, "run", side_effect=AssertionError("must not launch a build")):
            with self.assertRaises(ValueError):
                installer.install(prefix)

    def test_existing_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            (path / "user-file").write_text("preserve me")
            self.reject(path)
            self.assertEqual((path / "user-file").read_text(), "preserve me")

    def test_existing_file(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "file"
            path.write_text("preserve me")
            self.reject(path)
            self.assertEqual(path.read_text(), "preserve me")

    def test_dangling_symlink(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "link"
            path.symlink_to(Path(directory) / "absent")
            self.reject(path)
            self.assertTrue(path.is_symlink())

    def test_missing_parent(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "absent" / "sdk"
            self.reject(path)
            self.assertFalse(path.parent.exists())

    def test_unsupported_platform(self):
        with patch.object(installer.sys, "platform", "darwin"):
            self.reject(Path("/unused"))

    def test_unsupported_architecture(self):
        with patch.object(installer.platform, "machine", return_value="aarch64"):
            self.reject(Path("/unused"))


if __name__ == "__main__":
    unittest.main()
