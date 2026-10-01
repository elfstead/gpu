import copy
import json
import unittest
import frontier as f


class FrontierTests(unittest.TestCase):
    def fixture(self,capacity,strategy):
        device={'name':'synthetic'}
        features=dict(multiDrawIndirect=True,drawIndirectCount=True,maxDrawIndirectCount=512)
        stdout='Native indexed frontier drained\nDEVICE '+json.dumps(device)+'\nFRONTIER_FEATURES '+json.dumps(features)+'\n'
        stdout+=''.join('FRONTIER_FRAME '+json.dumps(r)+'\n' for r in f.frames(capacity))
        names=('vkQueueSubmit2','vkBeginCommandBuffer','vkEndCommandBuffer','vkCmdBeginRendering','vkCmdEndRendering',
               'vkCreateCommandPool','vkDestroyCommandPool','vkQueueWaitIdle','vkResetCommandPool',
               'vkCmdDrawIndexedIndirect2KHR','vkCmdDrawIndexedIndirectCount2KHR','indexed_records','counted_capacity')
        start=dict.fromkeys(names,0);start.update(phase=0,vkQueueSubmit2=1,vkBeginCommandBuffer=1,vkEndCommandBuffer=1,
                                                vkCreateCommandPool=1,vkDestroyCommandPool=1)
        end=start.copy();end['phase']=1
        for name in names[:7]: end[name]+=8
        end['vkCmdDrawIndexedIndirect2KHR']=8*capacity if strategy=='single' else 8 if strategy=='multi' else 0
        end['vkCmdDrawIndexedIndirectCount2KHR']=8 if strategy=='count' else 0
        end['indexed_records']=0 if strategy=='count' else 8*capacity
        end['counted_capacity']=8*capacity if strategy=='count' else 0
        final=dict(end,phase=2)
        memory=dict(allocations=9,frees=9,peak_count=9,live_count=0,live_bytes=0,peak_bytes=900)
        def stderr(cs=(start,end,final),m=memory):
            return ('MEMORY_SUMMARY '+json.dumps(m)+'\n'+''.join('COMMAND_COUNTS '+json.dumps(c)+'\n' for c in cs)+
                    ('ALLOCATE '+json.dumps(dict(bytes=100,type=0))+'\n')*9)
        return stdout,stderr,device,(start,end,final),memory

    def test_strategies_and_false_evidence(self):
        for capacity in (1,64,512):
            for strategy in f.STRATEGIES:
                out,err,device,counts,memory=self.fixture(capacity,strategy)
                f.check_log(out,err(),capacity,strategy,device)
                for field in counts[1]:
                    changed=copy.deepcopy(counts);changed[1][field]+=1
                    with self.assertRaises((ValueError,RuntimeError)):
                        f.check_log(out,err(changed),capacity,strategy,device)
                for corrupted in (out.replace('"active": 0','"active": 1'),out.replace('true','false'),
                                  out.replace('Native indexed frontier drained',''),out+'Validation Error: fake'):
                    with self.assertRaises((ValueError,RuntimeError)):
                        f.check_log(corrupted,err(),capacity,strategy,device)
                with self.assertRaises((ValueError,RuntimeError)):
                    f.check_log(out,err(m=dict(memory,frees=8)),capacity,strategy,device)

    def test_generated_layout_and_count_is_not_masked(self):
        for capacity in (1,64,512):
            for frame in f.frames(capacity):
                for strategy in f.STRATEGIES:
                    mesh=f.mesh_expected(frame,strategy)
                    self.assertEqual(len(mesh),548+20*capacity)
                    self.assertEqual(mesh[-64:],f.oracle.GUARD)
                    self.assertEqual(mesh[-68:-64],f.struct.pack('<I',frame['active']))
                    expected=6 if strategy=='count' or frame['active'] else 0
                    self.assertEqual(f.struct.unpack_from('<I',mesh,480)[0],expected)

    def test_count_followup_requires_both_native_commands(self):
        out,err,device,counts,_=self.fixture(64,'count')
        for row in counts[1:]:
            row['vkCmdDrawIndexedIndirect2KHR']=8
            row['indexed_records']=512
        f.check_log(out,err(),64,'count-fixed',device)
        for key in ('vkCmdDrawIndexedIndirect2KHR','vkCmdDrawIndexedIndirectCount2KHR','indexed_records','counted_capacity'):
            bad=copy.deepcopy(counts);bad[1][key]-=1
            with self.assertRaises((ValueError,RuntimeError)):
                f.check_log(out,err(bad),64,'count-fixed',device)

    def test_identity_oracle_preserves_depth_guards_and_exposes_grouping(self):
        image=f.oracle.GUARD+f.oracle.BLACK+f.oracle.RED+f.oracle.GREEN+f.oracle.GUARD*2+bytes(range(12))+f.oracle.GUARD
        single=f.identity_image(image,3,1,512,'single')
        multi=f.identity_image(image,3,1,512,'multi')
        self.assertEqual(single[:64],image[:64]);self.assertEqual(single[76:],image[76:])
        self.assertEqual(multi[:64],image[:64]);self.assertEqual(multi[76:],image[76:])
        self.assertEqual(single[64:76],f.oracle.BLACK+bytes((2,0,255,255,3,0,255,255)))
        self.assertEqual(multi[64:76],f.oracle.BLACK+bytes((2,0,255,255,1,4,255,255)))
        self.assertEqual(multi,f.identity_image(image,3,1,512,'count'))
        for strategy in f.STRATEGIES:
            self.assertEqual(single,f.identity_image(image,3,1,1,strategy))
        with self.assertRaises((ValueError,RuntimeError)):
            f.identity_image(image[:64]+bytes((1,1,1,1))+image[68:],3,1,512,'multi')

    def test_identity_feature_is_explicit(self):
        out,err,device,_,_=self.fixture(64,'multi')
        with self.assertRaises((ValueError,RuntimeError)):
            f.check_log(out,err(),64,'multi',device,True)
        out+='DRAW_IDENTITY_FEATURES {"shaderDrawParameters":true}\n'
        f.check_log(out,err(),64,'multi',device,True)
        with self.assertRaises((ValueError,RuntimeError)):
            f.check_log(out,err(),64,'multi',device)

    def test_public_cache_policy_is_not_mistaken_for_owned_reuse(self):
        for capacity in (1,64,512):
            for strategy in f.STRATEGIES:
                out,err,device,commands,_=self.fixture(capacity,strategy)
                out=out.replace('Native indexed','Public indexed')
                uncached=capacity==512 and strategy=='single'
                start,end,final=copy.deepcopy(commands)
                start.update(vkDestroyCommandPool=0,vkResetCommandPool=1)
                end.update(vkCreateCommandPool=9 if uncached else 1,vkDestroyCommandPool=8 if uncached else 0,
                           vkResetCommandPool=1 if uncached else 9)
                final.update(vkCreateCommandPool=end['vkCreateCommandPool'],vkDestroyCommandPool=end['vkDestroyCommandPool']+1,
                             vkResetCommandPool=end['vkResetCommandPool'])
                f.check_log(out,err((start,end,final)),capacity,strategy,device,public=True)
                final['vkCreateCommandPool']+=1
                with self.assertRaises((ValueError,RuntimeError)):
                    f.check_log(out,err((start,end,final)),capacity,strategy,device,public=True)


if __name__=='__main__': unittest.main()
