#!/usr/bin/env python3
"""Matched one/two-slot indexed scene correctness; validation/tracing, NOT timing."""
import argparse
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
sys.dont_write_bytecode=True
import run as scene
import oracle
HERE=scene.HERE
ROOT=scene.ROOT
require=scene.generate.require
digest=scene.digest


def rows(text,prefix):
    return [json.loads(line[len(prefix):]) for line in text.splitlines() if line.startswith(prefix)]


def check_run(stdout,stderr,config,device):
    backend,bits,mode,slots,strategy,frames,width,height=config
    require('Validation Error:' not in stdout+stderr and 'SYNC-HAZARD' not in stdout+stderr,'validation failed')
    require(f'{backend.title()} scene reuse drained and checked' in stdout,'missing drain marker')
    require(rows(stdout,'DEVICE ')==[device],'device mismatch')
    expected=[]
    for f in range(frames):
        generation=f//slots;variant=generation%4
        expected.append(dict(frame=f,slot=f%slots,generation=generation,phase=int(variant==1),empty=int(variant==2)))
    require(rows(stdout,'REUSE_FRAME ')==expected,'frame/slot/generation mismatch')
    encodes=slots if strategy=='replay' else frames
    wanted=dict(frames=frames,slots=slots,mode=mode,index_bytes=bits//8,replay=int(strategy=='replay'),
                encodes=encodes,peak_unretired=slots,requested_bytes=slots*(width*height*16+1816))
    require(rows(stdout,'REUSE_SUMMARY ')==[wanted],'reuse summary mismatch')
    memory=rows(stderr,'MEMORY_SUMMARY ')
    require(len(memory)==1,'missing memory summary')
    memory=memory[0]
    require(memory['allocations']==memory['frees']==memory['peak_count']==9*slots and
            memory['live_count']==memory['live_bytes']==0,'allocation lifetime/count mismatch')
    counts=rows(stderr,'COMMAND_COUNTS ')
    require([r['phase'] for r in counts]==[0,1,2],'missing command snapshots')
    start,end,final=counts
    setup_recordings=slots+(slots if strategy=='replay' else 0)
    require(start['vkBeginCommandBuffer']==start['vkEndCommandBuffer']==setup_recordings
        and start['vkQueueSubmit2']==slots,'wrong setup recording/submission count')
    hot={k:end[k]-start[k] for k in start if k!='phase'}
    require(all(v>=0 for v in hot.values()),'command counter decreased')
    require(final['vkCreateCommandPool']==final['vkDestroyCommandPool'],'command pool leak')
    require(final['vkQueueWaitIdle']==0,'successful path used queue-idle')
    require(hot['vkQueueSubmit2']==frames,'wrong submission count')
    recorded=0 if strategy=='replay' else frames
    require(hot['vkBeginCommandBuffer']==hot['vkEndCommandBuffer']==recorded,'not actual reset/replay')
    require(hot['vkCmdDrawIndexedIndirect2KHR']==2*recorded,'wrong native indexed draw count')
    scopes=1 if mode==0 else 2
    require(hot['vkCmdBeginRendering']==hot['vkCmdEndRendering']==scopes*recorded,'wrong scope grouping')
    if strategy=='replay':
        require(hot['vkResetCommandPool']==hot['vkCreateCommandPool']==hot['vkDestroyCommandPool']==0,'replay changed command storage')
    else:
        require(hot['vkCreateCommandPool']==slots and hot['vkDestroyCommandPool']==0,'reset path allocated per-frame pools')
        require(hot['vkResetCommandPool']==(frames if backend=='public' else frames-slots),'wrong reset count')
    allocations=rows(stderr,'ALLOCATE ')
    require(len(allocations)==9*slots,'missing allocation events')
    return dict(summary=wanted,memory=memory,commands=counts,hot_commands=hot,
                allocation_shape=sorted((a['bytes'],a['type']) for a in allocations))


def reference(path,bits,extents):
    report=json.loads(path.read_text())
    require(report.get('complete') and not report.get('dirty') and not report.get('build_only')
            and report.get('schema') in (2,3) and report.get('index_bytes',4)==bits//8,'reference is not clean accepted scene')
    selected={}
    for width,height in extents:
        matches=[r for r in report['runs'] if r.get('backend')=='native' and r['extent']==[width,height]]
        require(len(matches)==1,'reference extent missing/duplicated')
        run=matches[0];directory=path.parent/run['directory']
        require(len(run['checks'])==30,'reference frame matrix incomplete')
        checked=[]
        # Re-evaluate all analytic expectations and guards, not just a PASS flag.
        for mode in range(10):
            for frame in range(3):
                artifacts=[]
                for suffix in ('images','geometry'):
                    name=f'mode-{mode}-frame-{frame}.{suffix}'
                    require(digest(directory/name)==run['files'][name],'reference artifact hash mismatch')
                    artifacts.append((directory/name).read_bytes())
                checked.append(oracle.check(*artifacts,width,height,int(frame==1),mode,bits//8))
        selected[(width,height)]=dict(directory=str(directory.resolve()),device=run['device'],checks=checked)
    return dict(path=str(path.resolve()),sha256=digest(path),revision=report['revision']),selected


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check',action='store_true')
    parser.add_argument('--preflight',action='store_true',help='8 frames, development only')
    parser.add_argument('--software',action='store_true',help='64 frames instead of 1000')
    parser.add_argument('--scale',action='store_true',help='also 8-frame 720p A/B/empty/A per-slot checks')
    parser.add_argument('--reference32',type=Path)
    parser.add_argument('--reference16',type=Path)
    args=parser.parse_args()
    base=ROOT/'target/graphics-scene';base.mkdir(parents=True,exist_ok=True)
    dest=Path(tempfile.mkdtemp(prefix='reuse-',dir=base));print(f'Reuse evidence: {dest}',flush=True)
    report=dict(schema=1,complete=False,scope='matched indexed scene reuse correctness; no timing or full M4 acceptance',
                revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
                dirty=bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True)),runs=[],references={},
                preflight=args.preflight,software=args.software,scale=args.scale)
    def save(): (dest/'report.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
    save()
    try:
        require(not os.getenv('CARGO_TARGET_DIR'),'leave CARGO_TARGET_DIR unset')
        compiler=shlex.split(os.getenv('CC','cc'));slang=os.getenv('SLANGC','slangc')
        report['toolchain']=dict(cc=subprocess.check_output([*compiler,'--version'],text=True).splitlines()[0],
                                slang=scene.generate.VERSION,python=sys.version.split()[0])
        sources=[p for p in HERE.iterdir() if p.suffix in ('.c','.h','.py','.slang','.md')]
        sources += [ROOT/'examples/learned_image'/n for n in ('native.c','native_workload.h','extent.h','trace_memory.c','allocation_tracker.h')]
        sources += list((ROOT/'examples/learned_image/generated').glob('*.h'))+list((ROOT/'examples/compiler').glob('*.py'))
        sources += [ROOT/'include/ogpu.h',ROOT/'vendor/Vulkan-Headers/include/vulkan/vulkan_core.h']
        report['sources']={str(p.relative_to(ROOT)):digest(p) for p in sorted(sources)}
        report['shaders']={}
        for bits in (16,32):
            artifacts=dest/f'shaders{bits}';artifacts.mkdir()
            for name,stage in [('prepare_reuse16' if bits==16 else 'prepare_reuse','compute'),('scene.vert','vertex'),('scene.frag','fragment')]:
                reflected,assembly,binary=scene.generate.compile_source(HERE/(name+'.slang'),artifacts/stage,slang,stage)
                if stage!='fragment':
                    parameters=reflected['parameters'];require(len(parameters)==1,'wrong root count')
                    root=parameters[0]['type']['elementType']
                    fields=[(f['name'],f['binding']['offset'],f['binding']['size']) for f in root['fields']]
                    expected=[('vertices',0,8)]+([('indices',8,8),('draws',16,8),('control',24,8)] if stage=='compute' else [])
                    require(fields==expected and scene.generate.uniform_size(root)==(32 if stage=='compute' else 8,8),'root layout mismatch')
                if stage=='compute': require(reflected['entryPoints'][0]['threadGroupSize']==[8,1,1],'wrong workgroup')
                (artifacts/(stage+'.spv')).write_bytes(binary)
                report['shaders'][f'{bits}-{stage}']=dict(sha256=digest(artifacts/(stage+'.spv')),reflection=reflected)
        subprocess.run([os.getenv('CARGO','cargo'),'build','--locked','--release','-p','ogpu'],cwd=ROOT,check=True)
        common=[*compiler,'-std=c11','-O2','-Wall','-Wextra','-Werror','-I'+str(ROOT/'include')]
        native_includes=['-I'+str(ROOT/'vendor/Vulkan-Headers/include'),'-I'+str(ROOT/'examples/learned_image/generated')]
        report['builds']={}
        for backend in ('native','public'):
            command=common+(native_includes if backend=='native' else [])+[str(HERE/f'reuse_{backend}.c')]
            if backend=='public': command+=['-L'+str(ROOT/'target/release'),'-Wl,-rpath,'+str(ROOT/'target/release'),'-logpu']
            command+=['-ldl','-o',str(dest/backend)];subprocess.run(command,check=True)
            symbols=subprocess.check_output(['nm','-u',str(dest/backend)],text=True)
            require(('ogpu' not in symbols.lower()) if backend=='native' else ('ogpu_command_list_submit' in symbols and
                not any(line.split()[-1].startswith('vk') for line in symbols.splitlines() if line.split())),'consumer linkage mismatch')
            report['builds'][backend]=dict(command=command,sha256=digest(dest/backend))
            baseline_command=[str(HERE/f'{backend}.c') if word==str(HERE/f'reuse_{backend}.c') else
                              str(dest/f'baseline-{backend}') if word==str(dest/backend) else word for word in command]
            subprocess.run(baseline_command,check=True)
            report['builds'][f'baseline-{backend}']=dict(command=baseline_command,sha256=digest(dest/f'baseline-{backend}'))
        command=common+native_includes+['-shared','-fPIC',str(HERE/'reuse_trace.c'),'-ldl','-o',str(dest/'trace.so')]
        subprocess.run(command,check=True);report['builds']['trace']=dict(command=command,sha256=digest(dest/'trace.so'))
        report['runtime_sha256']=digest(ROOT/'target/release/libogpu.so')
        command=common+['-Wno-unused-function',str(HERE/'test_reuse.c'),'-ldl','-o',str(dest/'test-reuse')]
        subprocess.run(command,check=True)
        tested=subprocess.run([str(dest/'test-reuse')],capture_output=True,text=True,check=True)
        require('Reuse schedule/input guards CPU PASS' in tested.stdout,'missing C CPU test marker')
        report['builds']['cpu_test']=dict(command=command,sha256=digest(dest/'test-reuse'))
        subprocess.run([sys.executable,'-B',str(HERE/'test_reuse.py')],check=True)
        if args.check:
            report['build_only']=True;report['complete']=True;save();print('Reuse build/parser gates PASS; no GPU');return
        require(args.reference32 and args.reference16,'both accepted index-width references are required')
        require(os.getenv('VK_DRIVER_FILES') and 'VK_LAYER_KHRONOS_validation' in os.getenv('VK_INSTANCE_LAYERS','')
            and os.getenv('VK_LAYER_VALIDATE_SYNC')=='1' and not os.getenv('VK_LOADER_LAYERS_DISABLE'),'select one ICD and enable Vulkan/sync validation')
        extents=[(257,193)]+([(1280,720)] if args.scale else [])
        refs={}
        for bits,path in [(32,args.reference32),(16,args.reference16)]:
            metadata,selected=reference(path,bits,extents);report['references'][str(bits)]=dict(**metadata,runs=list(selected.values()));refs[bits]=selected
        env=os.environ.copy();loader=env.get('OGPU_VULKAN_LIBRARY','libvulkan.so.1')
        env.update(OGPU_TRACE_LOADER=loader,OGPU_VULKAN_LIBRARY=str(dest/'trace.so'),OGPU_SCENE_TRACE='1',
                   LD_LIBRARY_PATH=str(ROOT/'target/release')+':'+env.get('LD_LIBRARY_PATH',''))
        report['environment']={k:env.get(k) for k in ('VK_DRIVER_FILES','VK_ICD_FILENAMES','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC',
            'OGPU_TRACE_LOADER','OGPU_VULKAN_LIBRARY','OGPU_SCENE_TRACE','LD_LIBRARY_PATH')}
        # The reusable path shares refactored helpers with the serial consumers.
        # Re-run their entire original frame matrix against the accepted binaries.
        report['baseline']=[]
        expected_rows=[dict(mode=m,phase=int(f==1),frame=f) for m in range(10) for f in range(3)]
        for width,height in extents:
            for bits in (16,32):
                ref=refs[bits][(width,height)];reference_dir=Path(ref['directory'])
                for backend in ('native','public'):
                    name=f'baseline-{width}x{height}-{bits}-{backend}';out=dest/name;out.mkdir()
                    command=[str(dest/f'baseline-{backend}'),str(width),str(height),str(reference_dir.parent),str(out),str(bits)]
                    with (out/'stdout').open('w') as stdout,(out/'stderr').open('w') as stderr:
                        execution=subprocess.run(command,stdout=stdout,stderr=stderr,env=env,timeout=300)
                    stdout=(out/'stdout').read_text();stderr=(out/'stderr').read_text()
                    require(execution.returncode==0 and 'Validation Error:' not in stdout+stderr and 'SYNC-HAZARD' not in stdout+stderr,'serial regression failed')
                    require(rows(stdout,'DEVICE ')==[ref['device']] and rows(stdout,'SCENE_FRAME ')==expected_rows,'serial frame/device mismatch')
                    require(f'{backend.title()} indexed/depth frames drained' in stdout,'serial drain marker missing')
                    comparison=scene.compare_outputs(reference_dir,out,expected_rows)
                    memory=rows(stderr,'MEMORY_SUMMARY ')
                    require(len(memory)==1 and memory[0]['allocations']==memory[0]['frees']==8
                        and memory[0]['live_count']==memory[0]['live_bytes']==0,'serial allocation leak')
                    report['baseline'].append(dict(name=name,command=command,comparison=comparison,memory=memory[0],
                        logs={n:digest(out/n) for n in ('stdout','stderr')}));save()
        print('Refactored serial scene: complete native/public matrices match accepted references',flush=True)
        shapes={}
        for width,height in extents:
            frames=8 if args.preflight or width==1280 else 64 if args.software else 1000
            for bits in (16,32):
                ref=refs[bits][(width,height)]
                for mode in (0,6,7,8):
                    for slots in (1,2):
                        for strategy in ('reset','replay'):
                            for backend in ('native','public'):
                                name=f'{width}x{height}-{bits}-{mode}-{slots}-{strategy}-{backend}';out=dest/name;out.mkdir()
                                command=[str(dest/backend),str(width),str(height),str(dest/f'shaders{bits}'),ref['directory'],
                                         str(frames),str(mode),str(slots),strategy,str(bits)]
                                with (out/'stdout').open('w') as stdout,(out/'stderr').open('w') as stderr:
                                    execution=subprocess.run(command,stdout=stdout,stderr=stderr,env=env,timeout=300)
                                require(execution.returncode==0,f'execution failed: {out}')
                                result=check_run((out/'stdout').read_text(),(out/'stderr').read_text(),
                                    (backend,bits,mode,slots,strategy,frames,width,height),ref['device'])
                                key=(width,height,slots)
                                shape=(result['allocation_shape'],result['memory']['peak_bytes'])
                                if key in shapes: require(shape==shapes[key],'native/public allocation budgets differ')
                                else: shapes[key]=shape
                                report['runs'].append(dict(name=name,command=command,backend=backend,extent=[width,height],
                                    device=ref['device'],**result,logs={n:digest(out/n) for n in ('stdout','stderr')}))
                                save()
                            print(f'Reuse {width}x{height} u{bits} mode={mode} slots={slots} {strategy}: {frames} frames/consumer PASS',flush=True)
        report['complete']=True;save();print(f'Matched scene reuse PASS; no timing claim: {dest / "report.json"}')
    except Exception as error:
        report['error']=str(error);save();raise


if __name__=='__main__': main()
