import importlib.util
from pathlib import Path
import tempfile
import unittest

spec=importlib.util.spec_from_file_location('sdk_profile',Path(__file__).parent/'sdk/profile.py')
profile=importlib.util.module_from_spec(spec);spec.loader.exec_module(profile)


class ProfileTests(unittest.TestCase):
    def test_default_and_explicit_profiles(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary);args=['--source','shader.slang']
            self.assertEqual(profile.select(args,root),args+['--api','foundation'])
            self.assertEqual(args,['--source','shader.slang'])
            for explicit in (['--api','foundation'],['--api=foundation']):
                self.assertEqual(profile.select(args+explicit,root),args+explicit)
            for explicit in (['--api','installed'],['--api=installed']):
                with self.assertRaisesRegex(ValueError,'--legacy'):profile.select(args+explicit,root)
            (root/'include').mkdir();(root/'include/ogpu.h').touch()
            for explicit in (['--api','installed'],['--api=installed']):
                self.assertEqual(profile.select(args+explicit,root),args+explicit)
            # Last option follows argparse semantics; packaging never changes default.
            self.assertEqual(profile.select(args,root),args+['--api','foundation'])


if __name__=='__main__':unittest.main()
