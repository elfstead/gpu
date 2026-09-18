import copy
import json
import sys
import unittest
sys.dont_write_bytecode = True
import run_native


class NativeAccountingTests(unittest.TestCase):
    def fixture(self, resident):
        buffers = [dict(host=False, requested=128, allocated=256, type=0, flags=1) for _ in range(5)]
        buffers += [dict(host=True, requested=64, allocated=128, type=5, flags=14) for _ in range(3)]
        if resident:
            buffers.append(dict(buffers[0]))
        image = dict(logical=16, allocated=4096, type=0, flags=1)
        order = [*buffers[:8], image, *buffers[8:]]
        events = ['ALLOCATE ' + json.dumps(dict(id=i+1, bytes=a['allocated'], type=a['type'])) for i, a in enumerate(order)]
        events += ['FREE ' + json.dumps(dict(id=i+1)) for i in reversed(range(len(order)))]
        summary = dict(allocations=len(order), frees=len(order), peak_count=len(order),
                       peak_bytes=sum(a['allocated'] for a in order), live_bytes=0, live_count=0)
        events.append('MEMORY_SUMMARY ' + json.dumps(summary))
        metadata = dict(mode='resident' if resident else 'end-to-end', host_buffers=3*64,
                        device_buffers=(6 if resident else 5)*128, image_logical=16)
        return buffers, image, metadata, '\n'.join(events)

    def stdout(self, buffers, image):
        return '\n'.join('NATIVE_BUFFER ' + json.dumps(b) for b in buffers) + '\nNATIVE_IMAGE ' + json.dumps(image)

    def test_modes_and_engine_matching(self):
        for resident in (False, True):
            buffers, image, metadata, trace = self.fixture(resident)
            native = run_native.allocation_evidence(self.stdout(buffers, image), trace, metadata, 'native')
            ogpu = run_native.allocation_evidence('', trace, metadata, 'ogpu')
            self.assertEqual(native['signature'], ogpu['signature'])
            self.assertEqual(native['summary']['live_bytes'], 0)

    def test_reject_native_description_disagreement(self):
        buffers, image, metadata, trace = self.fixture(False)
        mutations = [lambda b: b.pop(), lambda b: b[0].update(requested=129),
                     lambda b: b[0].update(allocated=300), lambda b: b[0].update(flags=2),
                     lambda b: b[0].update(type=1), lambda b: b[0].update(host=True)]
        for mutate in mutations:
            altered = copy.deepcopy(buffers)
            mutate(altered)
            with self.assertRaises(ValueError):
                run_native.allocation_evidence(self.stdout(altered, image), trace, metadata, 'native')
        with self.assertRaises(ValueError):
            run_native.allocation_evidence(self.stdout(buffers, dict(image, logical=20)), trace, metadata, 'native')


if __name__ == '__main__':
    unittest.main()
