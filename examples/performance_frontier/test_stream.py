import importlib.util
import json
from pathlib import Path
import sys
import unittest
import tempfile
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("frontier_stream", Path(__file__).with_name("stream.py"))
stream = importlib.util.module_from_spec(spec); spec.loader.exec_module(stream)


class StreamEvidence(unittest.TestCase):
    def output(self):
        return '\n'.join((
            'DEVICE {"vendor":1,"device":2,"api":[1,4,0]}',
            'STREAM ' + json.dumps(dict(policy="replay", slots=2, extent=[65,47,131,95], frames=1,
                warmups=100, validation=False, setup_ms=1., wall_ms=2., max_rgb_delta=1)),
            'FRAME ' + json.dumps(dict(index=0, upload_ms=.1, record_ms=.1, submit_ms=.1,
                wait_ms=.1, read_ms=.1, latency_ms=.6)),
            'Streaming final input/weight integrity and all intermediate guards PASS',
            'Streaming full-frame pixels/readback guards PASS; all slots drained'))

    def parse(self, text):
        return stream.parse(text, "replay", 2, (65,47,131,95), 1, False)

    def test_complete(self):
        self.assertEqual(len(self.parse(self.output())[2]), 1)
        self.assertEqual(stream.summarize(self.parse(self.output())[2])["upload_ms"]["median"], .1)
        stream.parse(self.output().replace('"replay"', '"compiled"'), "compiled", 2, (65,47,131,95), 1, False)

    def test_invalid(self):
        for before, after in [('"max_rgb_delta": 1', '"max_rgb_delta": 2'), ('"slots": 2', '"slots": 3'),
                              ('"read_ms": 0.1', '"read_ms": NaN'), ('"latency_ms": 0.6', '"latency_ms": 0.1'),
                              ('Streaming final input/weight integrity and all intermediate guards PASS', '')]:
            with self.assertRaises(RuntimeError):
                self.parse(self.output().replace(before, after))

    def test_incomplete_samples(self):
        with self.assertRaises(RuntimeError):
            self.parse('\n'.join(x for x in self.output().splitlines() if not x.startswith('FRAME ')))

    def test_export_rejects_incomplete_and_preflight(self):
        spec = importlib.util.spec_from_file_location('stream_export', Path(__file__).with_name('export_stream.py'))
        exporter = importlib.util.module_from_spec(spec); spec.loader.exec_module(exporter)
        with tempfile.TemporaryDirectory(prefix='stream-export-test-') as directory:
            root = Path(directory); source = root / 'report.json'
            for report in (dict(schema=1, complete=False), dict(schema=1, complete=True, preflight=True),
                           dict(schema=1, complete=True, preflight=False, software=False, compiled_only=True),
                           dict(schema=1, complete=True, preflight=False, software=True, compiled_only=True, validation=[]),
                           dict(schema=1, complete=True, preflight=False, software=False, validation=[])):
                source.write_text(json.dumps(report))
                with self.assertRaises(RuntimeError):
                    exporter.export(source, root / 'out')
                self.assertFalse((root / 'out').exists())


if __name__ == '__main__':
    unittest.main()
