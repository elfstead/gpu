"""Reject wrong range command strategies, frame histories and storage budgets."""
import json
import unittest
import range_reuse as r


class RangeReuseTests(unittest.TestCase):
    def test_parser_matrix_and_corruption(self):
        for backend in ('native','public'):
            for replay in (False,True):
                for strategy in r.f.STRATEGIES:
                    for slots in (1,2):
                        capacity=64;frames=16;device={'vendor':1}
                        rows=[dict(frame=i,slot=i%slots,generation=i//slots,
                                   phase=(i//slots)%2,active=r.f.frames(capacity)[i//slots%8]['active']) for i in range(frames)]
                        encodes=slots if replay else frames;size=132+20*capacity
                        summary=dict(frames=frames,slots=slots,capacity=capacity,strategy=r.f.STRATEGIES.index(strategy),replay=int(replay),
                                     encodes=encodes,peak_unretired=slots,requested_bytes=slots*(257*193*16+1232+2*size+max(size,256)))
                        names=('vkCreateCommandPool','vkDestroyCommandPool','vkResetCommandPool','vkBeginCommandBuffer','vkEndCommandBuffer',
                               'vkQueueSubmit2','vkQueueWaitIdle','vkCmdBeginRendering','vkCmdEndRendering',
                               'vkCmdDrawIndexedIndirect2KHR','vkCmdDrawIndexedIndirectCount2KHR','indexed_records','counted_capacity',
                               'vkCmdBindPipeline','vkCmdPushDataEXT','vkCmdBindIndexBuffer3KHR','vkCmdPipelineBarrier2')
                        start=dict.fromkeys(names,0);start.update(phase=0,vkQueueSubmit2=slots,
                            vkBeginCommandBuffer=slots*(1+int(replay)),vkEndCommandBuffer=slots*(1+int(replay)),
                            vkCreateCommandPool=slots*(1+int(replay)),vkDestroyCommandPool=slots)
                        def draw_counts(n):
                            return dict(vkCmdDrawIndexedIndirect2KHR=0 if strategy=='count' else n*(capacity if strategy=='single' else 1),
                                        vkCmdDrawIndexedIndirectCount2KHR=n if strategy=='count' else 0,
                                        indexed_records=0 if strategy=='count' else n*capacity,
                                        counted_capacity=n*capacity if strategy=='count' else 0)
                        start.update(draw_counts(slots if replay else 0))
                        if replay:
                            start.update(vkCmdBindPipeline=slots*2,vkCmdPushDataEXT=slots*2,vkCmdBindIndexBuffer3KHR=slots)
                        hot=dict.fromkeys(names,0);hot['vkQueueSubmit2']=frames
                        if not replay:
                            binds=1
                            hot.update(vkCreateCommandPool=slots,vkResetCommandPool=frames if backend=='public' else frames-slots,
                                       vkBeginCommandBuffer=frames,vkEndCommandBuffer=frames,vkCmdBeginRendering=frames,vkCmdEndRendering=frames,
                                       vkCmdBindPipeline=frames*(1+binds),vkCmdPushDataEXT=frames*(1+binds),
                                       vkCmdBindIndexBuffer3KHR=frames*binds,vkCmdPipelineBarrier2=frames,
                                       **draw_counts(frames))
                        end={k:start[k]+hot[k] for k in names};end['phase']=1
                        final=dict(end,phase=2);final['vkDestroyCommandPool']=final['vkCreateCommandPool']
                        if backend=='public' and replay:final['vkResetCommandPool']+=slots
                        memory=dict(allocations=9*slots,frees=9*slots,peak_count=9*slots,live_count=0,live_bytes=0,peak_bytes=2000000)
                        def logs():
                            stdout='DEVICE '+json.dumps(device)+'\n'+''.join('RANGE_FRAME '+json.dumps(row)+'\n' for row in rows)
                            stdout+=('PUBLIC_SCENE '+json.dumps(dict(abi=19,index_bytes=4,gpu_generated=True)) if backend=='public'
                                     else 'DRAW_IDENTITY_FEATURES '+json.dumps(dict(shaderDrawParameters=True)))+'\n'
                            stdout+='RANGE_SUMMARY '+json.dumps(summary)+f'\n{backend.title()} scene reuse drained and checked\n'
                            stderr='MEMORY_SUMMARY '+json.dumps(memory)+'\n'+''.join('COMMAND_COUNTS '+json.dumps(c)+'\n' for c in (start,end,final))
                            stderr+=('ALLOCATE '+json.dumps(dict(bytes=100,type=0))+'\n')*(9*slots)
                            return stdout,stderr
                        def check():r.check(*logs(),backend,capacity,strategy,slots,replay,frames,device)
                        check()
                        for target,key in ((summary,'encodes'),(summary,'requested_bytes'),(summary,'peak_unretired'),
                            (rows[3],'active'),(rows[3],'generation'),(memory,'live_count'),
                            (end,'vkCmdDrawIndexedIndirect2KHR'),(end,'vkCmdDrawIndexedIndirectCount2KHR'),
                            (end,'indexed_records'),(end,'counted_capacity'),(end,'vkCreateCommandPool'),
                            (end,'vkCmdBindPipeline'),(end,'vkCmdPushDataEXT'),(end,'vkCmdBindIndexBuffer3KHR'),
                            (final,'vkResetCommandPool'),(final,'vkQueueWaitIdle'),(final,'vkDestroyCommandPool')):
                            target[key]+=1
                            with self.assertRaises(ValueError):check()
                            target[key]-=1
                        # A replay with extra compilation-time bindings must fail
                        # even when its hot binding deltas remain exactly zero.
                        if replay:
                            for key in ('vkCmdBindPipeline','vkCmdPushDataEXT','vkCmdBindIndexBuffer3KHR'):
                                for trace in (start,end,final):trace[key]+=1
                                with self.assertRaises(ValueError):check()
                                for trace in (start,end,final):trace[key]-=1
                        out,err=logs()
                        with self.assertRaises(ValueError):r.check(out,err+'Validation Error:',backend,capacity,strategy,slots,replay,frames,device)


if __name__=='__main__':unittest.main()
