import collections
import json
from pathlib import Path
import sys
import tempfile
import unittest
sys.dont_write_bytecode = True
import compare_native as compare
import export_comparison
import run_native
from test_benchmark import MeasurementTests


class PairedTests(unittest.TestCase):
    def test_balanced_engine_order(self):
        first = collections.Counter()
        keys = []
        for round_index in range(3):
            for extent_index, extent in enumerate(compare.bench.EXTENTS):
                for mode_index, mode in enumerate(compare.bench.MODES if round_index % 2 == 0 else tuple(reversed(compare.bench.MODES))):
                    engines = compare.control_order(round_index, extent_index, mode_index)
                    self.assertEqual(set(engines), {'ogpu', 'native'})
                    first[engines[0]] += 1
                    keys.extend((extent, engine, mode, round_index) for engine in engines)
        self.assertEqual(first, dict(ogpu=12, native=12))
        self.assertEqual(len(keys), 48)
        self.assertEqual(len(set(keys)), 48)

    def test_timing_has_no_validation_or_tracing(self):
        original = dict(VK_INSTANCE_LAYERS='VK_LAYER_KHRONOS_validation', VK_LAYER_VALIDATE_SYNC='1',
                        VK_LOADER_LAYERS_ENABLE='*', OGPU_VULKAN_LIBRARY='real-loader')
        result = compare.timing_environment(original)
        self.assertEqual(result, dict(VK_LOADER_LAYERS_DISABLE='*', OGPU_VULKAN_LIBRARY='real-loader'))
        self.assertIn('VK_INSTANCE_LAYERS', original)
        with self.assertRaises(ValueError):
            compare.timing_environment(dict(original, OGPU_TRACE_LOADER='real-loader'))

    def test_clock_and_identity(self):
        identity = dict(device=dict(vendor=4098, device=29471, api=[1,4,354]),
                        clock=dict(supported=True, bits=64, period_ns=10))
        encoded = 'DEVICE ' + json.dumps(identity['device']) + '\nCLOCK ' + json.dumps(identity['clock'])
        self.assertEqual(run_native.identity_evidence(encoded), identity)
        for clock in (dict(supported=True, bits=35, period_ns=10), dict(supported=True, bits=64, period_ns=0),
                      dict(supported=False, bits=64, period_ns=10), dict(supported=True, bits=64, period_ns=float('nan'))):
            with self.assertRaises(ValueError):
                run_native.identity_evidence('DEVICE {}\nCLOCK ' + json.dumps(clock))
        with self.assertRaises(ValueError):
            run_native.identity_evidence(encoded + '\n' + encoded)

    def test_sample_identity_and_support_gates(self):
        fixture = MeasurementTests()
        metadata, samples = fixture.fixture()
        metadata.update(device_buffers=1184, host_buffers=632, cpu_payload=1584,
                        image_logical=4, upload_bytes=0, readback_bytes=0)
        identity = dict(device=dict(vendor=1, device=2, api=[1,4,0]), clock=dict(supported=True, bits=64, period_ns=1))
        text = fixture.encoded(metadata, samples) + '\nDEVICE ' + json.dumps(identity['device']) + '\nCLOCK ' + json.dumps(identity['clock'])
        compare.checked_samples(text, (1,1,1,1), 'resident', identity)
        with self.assertRaises(ValueError):
            compare.checked_samples(text, (1,1,1,1), 'resident', dict(identity, device={}))
        unsupported = dict(identity, clock=dict(supported=False, bits=0, period_ns=0))
        text = fixture.encoded(metadata, samples) + '\nDEVICE ' + json.dumps(identity['device']) + '\nCLOCK ' + json.dumps(unsupported['clock'])
        with self.assertRaises(ValueError):
            compare.checked_samples(text, (1,1,1,1), 'resident', unsupported)

    def test_export_requires_complete_matrix(self):
        with tempfile.TemporaryDirectory() as directory:
            report, destination = Path(directory) / 'report.json', Path(directory) / 'export'
            for value in ({}, dict(complete=False), dict(schema=1, complete=True, runs=[])):
                report.write_text(json.dumps(value))
                with self.assertRaises(ValueError):
                    export_comparison.export(report, destination)
                self.assertFalse(destination.exists())


if __name__ == '__main__':
    unittest.main()
