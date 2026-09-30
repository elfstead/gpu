"""Reject incomplete, mislabeled or leaky reuse runs without a GPU."""
import copy
import json
import unittest
import reuse


class ReuseTests(unittest.TestCase):
    def test_parser_gates(self):
        for backend in ('native','public'):
            for strategy in ('reset','replay'):
                config=(backend,16,6,2,strategy,8,257,193);device=dict(vendor=1,device=2,api=[1,4,354])
                frames=[dict(frame=f,slot=f%2,generation=f//2,phase=int(f//2%4==1),empty=int(f//2%4==2)) for f in range(8)]
                summary=dict(frames=8,slots=2,mode=6,index_bytes=2,replay=int(strategy=='replay'),encodes=2 if strategy=='replay' else 8,
                             peak_unretired=2,requested_bytes=2*(257*193*16+1816))
                memory=dict(allocations=18,frees=18,peak_count=18,live_count=0,live_bytes=0,peak_bytes=2000000)
                names=['vkCreateCommandPool','vkDestroyCommandPool','vkResetCommandPool','vkBeginCommandBuffer','vkEndCommandBuffer',
                       'vkQueueSubmit2','vkQueueWaitIdle','vkCmdBeginRendering','vkCmdEndRendering','vkCmdDrawIndexedIndirect2KHR']
                start=dict.fromkeys(names,0);start['phase']=0
                start['vkBeginCommandBuffer']=start['vkEndCommandBuffer']=4 if strategy=='replay' else 2
                start['vkQueueSubmit2']=2
                hot=dict.fromkeys(names,0);hot['vkQueueSubmit2']=8
                if strategy=='reset':
                    hot.update(vkCreateCommandPool=2,vkResetCommandPool=8 if backend=='public' else 6,
                        vkBeginCommandBuffer=8,vkEndCommandBuffer=8,vkCmdBeginRendering=16,vkCmdEndRendering=16,vkCmdDrawIndexedIndirect2KHR=16)
                end={k:start[k]+v for k,v in hot.items()};end['phase']=1
                final=dict(end,phase=2);final['vkDestroyCommandPool']=final['vkCreateCommandPool']
                def logs():
                    stdout='DEVICE '+json.dumps(device)+'\n'+''.join('REUSE_FRAME '+json.dumps(f)+'\n' for f in frames)
                    stdout+='REUSE_SUMMARY '+json.dumps(summary)+f'\n{backend.title()} scene reuse drained and checked\n'
                    stderr='MEMORY_SUMMARY '+json.dumps(memory)+'\n'+''.join('COMMAND_COUNTS '+json.dumps(c)+'\n' for c in (start,end,final))
                    stderr+=''.join('ALLOCATE '+json.dumps(dict(bytes=100,type=0))+'\n' for _ in range(18))
                    return stdout,stderr
                reuse.check_run(*logs(),config,device)
                mutations=[(frames[3],'phase',1-frames[3]['phase']),(summary,'peak_unretired',1),(memory,'live_count',1),
                           (final,'vkDestroyCommandPool',99),(end,'vkBeginCommandBuffer',99),(end,'vkQueueSubmit2',9),
                           (final,'vkQueueWaitIdle',1),(summary,'encodes',99)]
                for target,key,value in mutations:
                    original=copy.deepcopy(target[key]);target[key]=value
                    with self.assertRaises(ValueError): reuse.check_run(*logs(),config,device)
                    target[key]=original
                stdout,stderr=logs()
                with self.assertRaises(ValueError): reuse.check_run(stdout,stderr+'Validation Error:',config,device)
                frames.pop()
                with self.assertRaises(ValueError): reuse.check_run(*logs(),config,device)


if __name__=='__main__': unittest.main()
