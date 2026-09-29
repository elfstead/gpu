import importlib.util
import json
from pathlib import Path
import unittest
spec = importlib.util.spec_from_file_location("reuse_failures", Path(__file__).with_name("failures.py"))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class FailureEvidenceTests(unittest.TestCase):
    def fixture(self, mode):
        context = dict(abi=18,backend="vulkan",workload="learned-image",mode=mode,allocation="arena",policy="compiled",slots=2)
        context.update({k:1 for k in ("graphics","compute","buffer_address","timeline","synchronization2",
                                     "descriptor_heap","address_commands","untyped_pointers")})
        def event(slot, operation, state, result=0, native_result=0, pending=False, generation=7):
            return dict(slot=slot,frame=12+slot,generation=generation,state=state,pending_receipt=pending,
                        retains_list=True,operation=operation,result=result,native_result=native_result,message="test")
        events = [event(0,"submit" if mode in (1,6) else "poll" if mode in (2,3) else "wait",
                        "recorded" if mode in (1,6) else "pending",0 if mode==2 else -5,
                        {1:-1,2:0,3:-1,4:-1,5:-4,6:-13}[mode],mode not in (1,6))]
        if mode == 1:
            events += [event(0,"known-unsubmitted-abort","idle"),event(0,"submit","recorded",generation=8)]
        recovered = mode <= 3
        if not recovered:
            for slot in range(2): events += [event(slot,"quarantine","failed-pending"),event(slot,"drained-no-recycle","failed-drained")]
        events += [event(i,"before-teardown","idle" if recovered else "failed-drained") for i in range(2)]
        gate = dict(recovered=recovered,verified_recovery_frames=64) if recovered else dict(recovered=False,outputs_after_error_accepted=False)
        stdout = ["REUSE_CONTEXT "+json.dumps(context),'DEVICE {"vendor":1}',"REUSE_FAILURE_GATE "+json.dumps(gate),
                  "Reuse failure/drain integration PASS; synthetic faults, not hardware-loss evidence",
                  "Reuse HOST padding and all slot generations retired PASS",
                  "Streaming final input/weight integrity and all intermediate guards PASS"]
        stdout += ["REUSE_EVENT "+json.dumps(e) for e in events]
        for slot in range(2):
            for resource in range(7):
                offset = (slot*5+resource)*64 if resource < 5 else slot*64
                stdout.append('REUSE_RANGE '+json.dumps(dict(slot=slot,resource=resource,
                    backing=0 if resource<5 else resource-4,offset=offset,size=64,end=offset+64,atom=1,host=resource>=5)))
        native = [dict(operation=runner.FAULT_OPERATIONS[mode-1],mode=mode,result=0,accepted=10,drained=10 if mode==5 else 9)]
        if mode == 5: native.insert(0,dict(operation="queue-drain",mode=5,result=0,accepted=10,drained=10))
        if mode in (4,6): native.append(dict(operation="real-wait" if mode==4 else "queue-drain",mode=0,result=0,accepted=10,drained=10))
        stderr = ["FAULT "+json.dumps(n) for n in native]
        stderr += ["FAULT_SUMMARY "+json.dumps(dict(injections=1,armed=0,accepted=10,drained=10,rejected_value=9 if mode==1 else 0))]
        stderr += ['ALLOCATE '+json.dumps(dict(id=i,bytes=100,type=0)) for i in range(1,7)]
        stderr += ['FREE '+json.dumps(dict(id=i)) for i in range(1,7)]
        stderr += ['MEMORY_SUMMARY '+json.dumps(dict(allocations=6,frees=6,peak_bytes=600,peak_count=6,live_bytes=0,live_count=0))]
        return '\n'.join(stdout),'\n'.join(stderr)

    def parse(self, mode, change=None):
        stdout,stderr = self.fixture(mode)
        if change: stdout,stderr = change(stdout,stderr)
        return runner.parse(stdout,stderr,mode,"arena","compiled",2)

    def test_all_modes(self):
        for mode in range(1,7): self.assertEqual(self.parse(mode)["context"]["mode"],mode)

    def test_reject_invalid_context_state_and_error(self):
        for old,new in [('"abi": 18','"abi": 17'),('"timeline": 1','"timeline": 0'),
                        ('"retains_list": true','"retains_list": false'),('"native_result": -1','"native_result": -4'),
                        ('"state": "pending"','"state": "idle"'),('"verified_recovery_frames": 64','"verified_recovery_frames": 0')]:
            with self.subTest(old=old), self.assertRaises(RuntimeError):
                self.parse(3,lambda out,err:(out.replace(old,new),err))

    def test_reject_missing_drain_or_quarantine(self):
        for mode in (4,5,6):
            with self.subTest(mode=mode), self.assertRaises(RuntimeError):
                self.parse(mode,lambda out,err:(out,err.replace('"drained": 10','"drained": 9')))
            with self.subTest(mode=mode), self.assertRaises(RuntimeError):
                self.parse(mode,lambda out,err:(out.replace('"state": "failed-pending"','"state": "idle"'),err))

    def test_reject_loss_before_drain(self):
        with self.assertRaises(RuntimeError):
            self.parse(5,lambda out,err:(out,err.replace('"operation": "queue-drain"','"operation": "wrong"')))

    def test_reject_missing_injection_and_leak(self):
        for old,new in [('"injections": 1','"injections": 0'),('"armed": 0','"armed": 1'),('FREE {"id": 1}','')]:
            with self.subTest(old=old), self.assertRaises((RuntimeError,ValueError)):
                self.parse(2,lambda out,err:(out,err.replace(old,new)))

    def test_reject_retry_without_new_generation(self):
        with self.assertRaises(RuntimeError):
            self.parse(1,lambda out,err:(out.replace('"generation": 8','"generation": 7'),err))

    def test_reject_invalid_range(self):
        for old,new in [('"atom": 1','"atom": 0'),('"offset": 64','"offset": 0'),('"backing": 0','"backing": 1')]:
            with self.subTest(old=old), self.assertRaises(RuntimeError):
                self.parse(2,lambda out,err:(out.replace(old,new),err))


if __name__ == "__main__": unittest.main()
