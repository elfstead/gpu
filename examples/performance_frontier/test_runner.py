import importlib.util
import json
from pathlib import Path
import unittest
import tempfile
import sys
sys.dont_write_bytecode = True

spec = importlib.util.spec_from_file_location("frontier", Path(__file__).with_name("run.py"))
frontier = importlib.util.module_from_spec(spec)
spec.loader.exec_module(frontier)


class EvidenceTests(unittest.TestCase):
    def output(self):
        return '\n'.join((
            'DEVICE {"vendor":1,"device":2,"api":[1,4,0]}',
            'RESULT ' + json.dumps(dict(policy="ogpu", slots=3, dispatches=64, frames=1,
                warmups=100, validation=False, requested_bytes=1164, setup_ms=1., wall_ms=2.)),
            'FRAME ' + json.dumps(dict(index=0, record_ms=.1, submit_ms=.2, wait_ms=.3, latency_ms=.7)),
            'Frontier full-output/guards PASS; all slots drained'))

    def parse(self, output):
        return frontier.parse(output, "ogpu", 3, 64, 1, False)

    def test_complete(self):
        _, _, frames = self.parse(self.output())
        self.assertAlmostEqual(frontier.summary(frames)["record_submit_ms"]["median"], .3)

    def test_missing_extra_samples(self):
        output = self.output()
        with self.assertRaises(RuntimeError):
            self.parse('\n'.join(line for line in output.splitlines() if not line.startswith('FRAME ')))
        with self.assertRaises(RuntimeError):
            self.parse(output + '\n' + next(line for line in output.splitlines() if line.startswith('FRAME ')))

    def test_policy_and_nonfinite(self):
        for before, after in [('"slots": 3', '"slots": 1'), ('"record_ms": 0.1', '"record_ms": NaN'),
                              ('"latency_ms": 0.7', '"latency_ms": 0.1')]:
            with self.assertRaises(RuntimeError):
                self.parse(self.output().replace(before, after))

    def test_order(self):
        for i in range(4):
            self.assertEqual(set(frontier.order(i)), set(frontier.POLICIES))
        self.assertEqual({frontier.order(i)[0] for i in range(4)}, set(frontier.POLICIES))

    def test_timing_environment(self):
        result = frontier.compare_native.timing_environment(dict(VK_INSTANCE_LAYERS='validation', VK_LAYER_VALIDATE_SYNC='1'))
        self.assertNotIn('VK_INSTANCE_LAYERS', result)
        self.assertEqual(result['VK_LOADER_LAYERS_DISABLE'], '*')

    def test_incomplete_export_rejected(self):
        export_spec = importlib.util.spec_from_file_location("frontier_export", Path(__file__).with_name("export.py"))
        exporter = importlib.util.module_from_spec(export_spec); export_spec.loader.exec_module(exporter)
        with tempfile.TemporaryDirectory(prefix='frontier-export-test-') as directory:
            root = Path(directory); source = root / 'report.json'
            for report in (dict(schema=1, complete=False), dict(schema=1, complete=True, software=False, validation=[])):
                source.write_text(json.dumps(report))
                with self.assertRaises(RuntimeError):
                    exporter.export(source, root / 'out')
                self.assertFalse((root / 'out').exists())


if __name__ == '__main__':
    unittest.main()
