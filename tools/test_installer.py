"""Safety checks: invalid/existing destinations must fail before invoking Cargo."""
import importlib.util
import json
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
    def test_package_profiles(self):
        # Exercise actual copy/manifest/publication logic without compiling Rust.
        for legacy in (False,True):
            with self.subTest(legacy=legacy),tempfile.TemporaryDirectory() as directory:
                root=Path(directory);library=root/'libogpu.so';library.write_bytes(b'test library')
                prefix=root/'sdk'
                package=dict(id='ogpu',name='ogpu',version='0.1.0',license='MIT',source=None)
                def fake_run(*args,**kwargs):
                    if args[1] in ('build','metadata'):
                        self.assertIn('--no-default-features',args)
                        self.assertEqual('--features' in args,legacy)
                        if legacy:self.assertEqual(args[args.index('--features')+1],'legacy')
                    if args[1]=='build':value=json.dumps(dict(reason='compiler-artifact',target=dict(name='ogpu'),filenames=[str(library)]))
                    elif args[1]=='metadata':value=json.dumps(dict(packages=[package],resolve=dict(nodes=[dict(id='ogpu',deps=[])])))
                    elif args[1]=='rev-parse':value='a'*40
                    elif args[1]=='status':value=''
                    elif args[1]=='--version':value='rustc test'
                    else:raise AssertionError(args)
                    return installer.subprocess.CompletedProcess(args,0,stdout=value,stderr='')
                with patch.object(installer,'run',side_effect=fake_run):installer.install(prefix,legacy=legacy)
                manifest=json.loads((prefix/'share/ogpu/manifest.json').read_text())
                self.assertEqual(manifest['default_api'],'foundation');self.assertEqual(manifest['legacy_sdk'],legacy)
                self.assertEqual('abi' in manifest,legacy)
                self.assertFalse(manifest['cargo_default_features'])
                self.assertEqual(manifest['cargo_features'],['legacy'] if legacy else [])
                self.assertEqual(manifest['foundation']['revision'],1)
                for path in ('include/ogpu_next.h','lib/pkgconfig/ogpu-foundation.pc',
                             'share/ogpu/examples/foundation-indexed-ranges/main.c','share/ogpu/tools/sdk_profile.py',
                             'share/ogpu/examples/foundation-textured-scene/main.c',
                             'share/ogpu/examples/foundation-textured-scene/textured_scene.generated.h'):
                    self.assertTrue((prefix/path).is_file(),path)
                for path in ('include/ogpu.h','lib/pkgconfig/ogpu.pc','share/ogpu/examples/indexed-scene/main.c',
                             'share/ogpu/examples/resource-reuse/main.c','share/ogpu/examples/transform/main.c'):
                    self.assertEqual((prefix/path).is_file(),legacy,path)
                for name,wanted in manifest['files_sha256'].items():
                    self.assertEqual(installer.hashlib.sha256((prefix/name).read_bytes()).hexdigest(),wanted,name)

    def reject(self, prefix):
        with patch.object(installer, "run", side_effect=AssertionError("must not launch a build")):
            for legacy in (False,True):
                with self.subTest(legacy=legacy),self.assertRaises(ValueError):
                    installer.install(prefix,legacy=legacy)

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
