import json
import unittest
import argument_patch as p


class ArgumentPatchTests(unittest.TestCase):
    def test_exact_ranges_and_accounting(self):
        for size in (64,256):
            for path in p.PATHS:
                updates=p.updates(path,size)
                total=sum(r['bytes'] for r in updates)
                self.assertEqual(total,128+(4*size+4 if path in ('native-partial','public-partial') else (5 if path=='public' else 8)*size))
                policy=dict(single_word=True,native_partial=path=='native-partial')
                commands=dict(phase=2,vkCmdPushDataEXT=len(updates),vkCmdDrawIndexedIndirect2KHR=8,indexed_records=8,
                    vkCmdDrawIndexedIndirectCount2KHR=0,counted_capacity=0,vkQueueWaitIdle=0,vkCmdBeginRendering=4,vkCmdEndRendering=4)
                memory=dict(allocations=10,frees=10,peak_count=10,live_count=0,live_bytes=0)
                def logs():
                    return ('ARGUMENT_PATCH_POLICY '+json.dumps(policy),
                        '\n'.join('ARGUMENT_PUSH '+json.dumps(r) for r in updates)+'\nCOMMAND_COUNTS '+json.dumps(commands)+
                        '\nMEMORY_SUMMARY '+json.dumps(memory)+'\n'+('ALLOCATE {"bytes":128,"type":0}\n'*10))
                p.check_trace(*logs(),path,size)
                for target,key in [(policy,'native_partial'),(commands,'vkCmdPushDataEXT'),(commands,'indexed_records'),
                                   (commands,'vkQueueWaitIdle'),(memory,'live_bytes')]+[(r,k) for r in updates for k in ('offset','bytes')]:
                    old=target[key];target[key]=not old if isinstance(old,bool) else old+1
                    with self.assertRaises(ValueError):p.check_trace(*logs(),path,size)
                    target[key]=old
        for path,size in [('public',8),('unknown',64)]:
            with self.assertRaises(ValueError):p.updates(path,size)


if __name__=='__main__':unittest.main()
