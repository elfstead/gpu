#!/usr/bin/env python3
"""Matched range/count/identity scenes with caller-owned storage and one/two-slot replay. No timing."""
import argparse
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
sys.dont_write_bytecode=True
import frontier as f
ROOT,HERE=f.ROOT,f.HERE
require,digest=f.require,f.digest


def check(stdout,stderr,backend,capacity,strategy,slots,replay,frames,device,width=257,height=193):
    require('Validation Error:' not in stdout+stderr and 'SYNC-HAZARD' not in stdout+stderr,'validation failed')
    require(backend.title()+' scene reuse drained and checked' in stdout,'missing drain marker')
    require(f.rows(stdout,'DEVICE ')==[device],'wrong device')
    if backend=='public':
        require(f.rows(stdout,'PUBLIC_SCENE ')==[dict(abi=19,index_bytes=4,gpu_generated=True)],'wrong public profile')
    else:
        require(f.rows(stdout,'DRAW_IDENTITY_FEATURES ')==[dict(shaderDrawParameters=True)],'native identity not enabled')
    expected=[]
    for frame in range(frames):
        generation=frame//slots;state=f.frames(capacity)[generation%8]
        expected.append(dict(frame=frame,slot=frame%slots,generation=generation,phase=state['phase'],active=state['active']))
    require(f.rows(stdout,'RANGE_FRAME ')==expected,'wrong slot/generation/count sequence')
    draws=132+20*capacity
    summary=dict(frames=frames,slots=slots,capacity=capacity,strategy=f.STRATEGIES.index(strategy),replay=int(replay),
                 encodes=slots if replay else frames,peak_unretired=slots,
                 requested_bytes=slots*(width*height*16+1232+2*draws+max(draws,256)))
    require(f.rows(stdout,'RANGE_SUMMARY ')==[summary],'wrong capacity/storage/slot budget')
    memory=f.rows(stderr,'MEMORY_SUMMARY ')
    require(len(memory)==1 and memory[0]['allocations']==memory[0]['frees']==memory[0]['peak_count']==9*slots
            and memory[0]['live_count']==memory[0]['live_bytes']==0,'allocation lifetime/count mismatch')
    commands=f.rows(stderr,'COMMAND_COUNTS ')
    require(len(commands)==3 and [c['phase'] for c in commands]==[0,1,2],'missing command traces')
    start,end,final=commands
    require(start['vkQueueSubmit2']==slots and start['vkBeginCommandBuffer']==start['vkEndCommandBuffer']==slots*(1+int(replay)),
            'wrong setup/replay compilation count')
    hot={k:end[k]-start[k] for k in start if k!='phase'}
    require(all(v>=0 for v in hot.values()),'counter decreased')
    require(hot['vkQueueSubmit2']==frames and final['vkQueueWaitIdle']==0,'submission/queue-idle mismatch')
    recorded=0 if replay else frames
    for name in ('vkBeginCommandBuffer','vkEndCommandBuffer','vkCmdBeginRendering','vkCmdEndRendering'):
        require(hot[name]==recorded,'wrong recording/scope policy')
    draws_per_record=capacity if strategy=='single' else 1
    require(hot['vkCmdDrawIndexedIndirect2KHR']==(0 if strategy=='count' else recorded*draws_per_record)
            and hot['vkCmdDrawIndexedIndirectCount2KHR']==(recorded if strategy=='count' else 0),'wrong native range command')
    require(hot['indexed_records']==(0 if strategy=='count' else recorded*capacity)
            and hot['counted_capacity']==(recorded*capacity if strategy=='count' else 0),'wrong record capacity')
    # Check compiled commands too, not just the zero hot-encoding replay delta.
    encodes=summary['encodes']
    require(end['vkCmdDrawIndexedIndirect2KHR']==(0 if strategy=='count' else encodes*draws_per_record)
            and end['vkCmdDrawIndexedIndirectCount2KHR']==(encodes if strategy=='count' else 0),'wrong compiled native range strategy')
    binds=1  # Consecutive identical draw state is bound once by both encoders.
    for name in ('vkCmdBindPipeline','vkCmdPushDataEXT'):
        require(hot[name]==recorded*(1+binds),'wrong compute/raster binding count')
        require(end[name]==encodes*(1+binds),'wrong compiled compute/raster binding count')
    require(hot['vkCmdBindIndexBuffer3KHR']==recorded*binds,'wrong index binding count')
    require(end['vkCmdBindIndexBuffer3KHR']==encodes*binds,'wrong compiled index binding count')
    require(hot['vkCmdPipelineBarrier2']==0 if replay else hot['vkCmdPipelineBarrier2']>=recorded,'missing/unexpected barriers')
    require(hot['vkCreateCommandPool']==(0 if replay else slots) and hot['vkDestroyCommandPool']==0,'hot pool churn')
    require(hot['vkResetCommandPool']==(0 if replay else frames if backend=='public' else frames-slots),'wrong reset policy')
    require(final['vkCreateCommandPool']==final['vkDestroyCommandPool'],'pool leak')
    require(final['vkResetCommandPool']==end['vkResetCommandPool']+(slots if backend=='public' and replay else 0),
            'wrong list-release storage reset count')
    require(all(final[k]==end[k] for k in end if k not in ('phase','vkDestroyCommandPool','vkResetCommandPool')),'unexpected teardown commands')
    allocations=f.rows(stderr,'ALLOCATE ');require(len(allocations)==9*slots,'missing allocation events')
    return dict(summary=summary,memory=memory[0],commands=commands,hot_commands=hot,
                allocation_shape=sorted((a['bytes'],a['type']) for a in allocations))


def select_cases(native,scale):
    extents=[(257,193)]+([(1280,720)] if scale else [])
    cases=[c for c in native['runs'] if tuple(c['extent']) in extents]
    keys=[(tuple(c['extent']),c['capacity'],c['strategy']) for c in cases]
    expected={(extent,capacity,strategy) for extent in extents for capacity in (1,64,512) for strategy in f.STRATEGIES}
    require(len(keys)==len(expected) and set(keys)==expected,'complete, unique native extent/capacity/strategy matrix required')
    return extents,cases


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native',required=True,type=Path)
    parser.add_argument('--preflight',action='store_true',help='16 frames per case, not sustained acceptance')
    parser.add_argument('--software',action='store_true',help='64 instead of 1000 frames per case')
    parser.add_argument('--scale',action='store_true',help='also 16-frame 720p count/identity cycles on Radeon')
    args=parser.parse_args()
    out=Path(tempfile.mkdtemp(prefix='range-reuse-',dir=ROOT/'target/graphics-scene'))
    print(f'Range reuse evidence: {out}',flush=True)
    report=dict(schema=1,complete=False,scope=__doc__,preflight=args.preflight,software=args.software,scale=args.scale,runs=[],
                revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
                dirty=bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True)))
    def save(): (out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    save()
    try:
        require(not os.getenv('CARGO_TARGET_DIR'),'leave CARGO_TARGET_DIR unset')
        require(not (args.scale and args.software),'useful-scale controls are selected for Radeon only')
        native=json.loads(args.native.read_text());directory=args.native.resolve().parent
        require(native.get('complete') and native.get('identity') and not native.get('dirty') and not native.get('build_only'),'clean identity reference required')
        extents,cases=select_cases(native,args.scale)
        require(digest(Path(native['reference']['path']))==native['reference']['sha256'],'changed serial reference')
        _,refs=f.reuse.reference(Path(native['reference']['path']),32,extents)
        for case in cases:
            width,height=case['extent'];serial=Path(refs[(width,height)]['directory'])
            for filename,h in {**case['files'],**case['logs']}.items():require(digest(directory/case['name']/filename)==h,'changed native reference')
            for frame in f.frames(case['capacity']):
                i=frame['frame'];active=min(frame['active'],case['capacity'])
                mode=9 if active==0 else 0 if case['capacity']>1 and active==case['capacity'] else 4
                expected=f.identity_image((serial/f'mode-{mode}-frame-{frame["phase"]}.images').read_bytes(),width,height,case['capacity'],case['strategy'])
                require((directory/case['name']/f'frame-{i}.images').read_bytes()==expected,'native analytic image mismatch')
                require((directory/case['name']/f'frame-{i}.geometry').read_bytes()==f.mesh_expected(frame,case['strategy']),'native geometry/count mismatch')
        report['native']=dict(path=str(args.native.resolve()),sha256=digest(args.native),revision=native['revision'])
        report['shaders']={}
        for stage,metadata in native['shaders'].items():
            require(digest(directory/(stage+'.spv'))==metadata['sha256'],'changed native shader')
            report['shaders'][stage]=metadata['sha256']
        subprocess.run(['cargo','build','--locked','--release','-p','ogpu'],cwd=ROOT,check=True)
        cc=shlex.split(os.getenv('CC','cc'));report['compiler']=subprocess.check_output([*cc,'--version'],text=True).splitlines()[0]
        common=[*cc,'-std=c11','-O2','-Wall','-Wextra','-Werror','-DSCENE_RANGE_REUSE','-I'+str(ROOT/'include')]
        report['builds']={}
        for backend in ('native','public'):
            command=common+[str(HERE/f'reuse_{backend}.c')]
            if backend=='public':command+=['-L'+str(ROOT/'target/release'),'-Wl,-rpath,'+str(ROOT/'target/release'),'-logpu']
            else:command+=['-DNATIVE_DRAW_IDENTITY','-I'+str(ROOT/'vendor/Vulkan-Headers/include'),'-I'+str(ROOT/'examples/learned_image/generated')]
            command+=['-ldl','-o',str(out/backend)];subprocess.run(command,check=True)
            symbols=subprocess.check_output(['nm','-u',str(out/backend)],text=True)
            require(('ogpu' not in symbols.lower()) if backend=='native' else 'ogpu_command_list_submit' in symbols,'incorrect runtime linkage')
            report['builds'][backend]=dict(command=command,sha256=digest(out/backend))
        command=[*cc,'-std=c11','-O2','-Wall','-Wextra','-Werror','-shared','-fPIC','-I'+str(ROOT/'vendor/Vulkan-Headers/include'),str(HERE/'reuse_trace.c'),'-ldl','-o',str(out/'trace.so')]
        subprocess.run(command,check=True);report['builds']['trace.so']=dict(command=command,sha256=digest(out/'trace.so'))
        command=[*cc,'-std=c11','-O2','-Wall','-Wextra','-Werror','-Wno-unused-function',str(HERE/'test_range_reuse.c'),'-ldl','-o',str(out/'test-range-reuse')]
        subprocess.run(command,check=True)
        with (out/'cpu-tests.txt').open('w') as log:subprocess.run([str(out/'test-range-reuse')],stdout=log,stderr=log,check=True)
        report['cpu_tests_sha256']=digest(out/'cpu-tests.txt')
        subprocess.run([sys.executable,'-B',str(HERE/'test_range_reuse.py')],check=True)
        sources=[p for p in HERE.iterdir() if p.suffix in ('.c','.h','.py','.slang')]
        sources+=list((ROOT/'crates/ogpu/src').glob('*.rs'))+[ROOT/'crates/vulkan-sys/src/bindings.rs',ROOT/'include/ogpu.h']
        sources+=list((ROOT/'examples/learned_image').glob('native*'))+list((ROOT/'examples/learned_image/generated').glob('*.h'))
        sources+=[ROOT/'examples/learned_image'/n for n in ('trace_memory.c','allocation_tracker.h')]
        report['sources']={str(p.relative_to(ROOT)):digest(p) for p in sorted(sources) if p.is_file()}
        report['runtime_sha256']=digest(ROOT/'target/release/libogpu.so')
        env=os.environ.copy()
        require(env.get('VK_DRIVER_FILES') and 'VK_LAYER_KHRONOS_validation' in env.get('VK_INSTANCE_LAYERS','') and env.get('VK_LAYER_VALIDATE_SYNC')=='1'
                and not env.get('VK_LOADER_LAYERS_DISABLE'),'select ICD and enable validation/sync')
        env.update(OGPU_TRACE_LOADER=env.get('OGPU_VULKAN_LIBRARY','libvulkan.so.1'),OGPU_VULKAN_LIBRARY=str(out/'trace.so'),OGPU_SCENE_TRACE='1')
        report['environment']={k:env.get(k) for k in ('VK_DRIVER_FILES','VK_ICD_FILENAMES','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC','OGPU_TRACE_LOADER')}
        for case in cases:
            width,height=case['extent']
            frames=16 if args.preflight or width==1280 else 64 if args.software else 1000
            for slots in (1,2):
                for policy in ('reset','replay'):
                    results=[]
                    for backend in ('native','public'):
                        name=f'{width}x{height}-{case["capacity"]}-{case["strategy"]}-{slots}-{policy}-{backend}';dest=out/name;dest.mkdir()
                        command=[str(out/backend),str(width),str(height),str(directory),str(directory/case['name']),str(frames),str(slots),policy,str(case['capacity']),case['strategy']]
                        with (dest/'stdout').open('w') as stdout,(dest/'stderr').open('w') as stderr:
                            execution=subprocess.run(command,env=env,stdout=stdout,stderr=stderr,timeout=300)
                        require(execution.returncode==0,f'execution failed: {dest}')
                        checked=check((dest/'stdout').read_text(),(dest/'stderr').read_text(),backend,case['capacity'],case['strategy'],slots,policy=='replay',frames,case['device'],width,height)
                        results.append(checked)
                        report['runs'].append(dict(name=name,backend=backend,extent=[width,height],command=command,device=case['device'],**checked,
                                                  logs={n:digest(dest/n) for n in ('stdout','stderr')}));save()
                    require(results[0]['allocation_shape']==results[1]['allocation_shape'] and results[0]['memory']==results[1]['memory'],'matched allocation budgets differ')
                    print(name.rsplit('-',1)[0]+': matched outputs, budgets and native reset/replay PASS',flush=True)
        report['complete']=True;save();print('Range reuse PASS; timing remains open')
    except Exception as error:report['error']=str(error);save();raise


if __name__=='__main__': main()
