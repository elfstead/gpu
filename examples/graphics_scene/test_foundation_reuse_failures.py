"""Fail-closed evidence parsing for multi-slot cleanup and uncertain completion."""
import json
import unittest
import foundation_reuse_failures as failures


class FailureTests(unittest.TestCase):
    def test_cleanup_and_fail_stop(self):
        for strategy in ('reset','replay'):
            for fault in failures.FAULTS:
                stdout='Public scene reuse drained and checked' if fault=='none' else ''
                stderr='' if fault=='none' else 'INJECT '+fault+'\n'
                unknown=fault in ('unknown','lost')
                if unknown:stderr+='Unknown completion; refusing unsafe teardown\n'
                else:
                    n=8 if fault=='flush1' else 17 if fault=='flush3' else 18
                    for prefix in ('CPU_MEMORY_SUMMARY ','MEMORY_SUMMARY '):
                        stderr+=prefix+json.dumps(dict(allocations=n,frees=n,live_count=0,live_bytes=0))+'\n'
                    submits={'none':10,'flush1':0,'flush3':1,'flush6':3,'end':2 if strategy=='replay' else 3,'submit':3,'wait':4}[fault]
                    stderr+='COMMAND_COUNTS '+json.dumps(dict(phase=2,vkCreateCommandPool=2,vkDestroyCommandPool=2,
                                                               vkQueueSubmit2=submits,vkQueueWaitIdle=0))+'\n'
                code=0 if fault=='none' else 1
                failures.check_fault(fault,strategy,code,stdout,stderr)
                bad=[(1-code,stdout,stderr),(code,stdout,stderr+'SYNC-HAZARD injected'),
                     (code,stdout,stderr+'runtime error: injected')]
                if unknown:bad.append((code,stdout,stderr+'TEARDOWN memory\n'))
                else:
                    bad.append((code,stdout,stderr.replace('"live_count": 0','"live_count": 1')))
                    bad.append((code,stdout,stderr.replace('"vkDestroyCommandPool": 2','"vkDestroyCommandPool": 1')))
                if fault!='none':bad.append((code,'SCENE_FRAME {}\nREUSE_FRAME {}',stderr))
                for args in bad:
                    with self.assertRaises(ValueError):failures.check_fault(fault,strategy,*args)


if __name__=='__main__':unittest.main()
