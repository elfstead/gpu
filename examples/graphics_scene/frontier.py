#!/usr/bin/env python3
"""Native indexed multi-record/count correctness frontier; deliberately NOT timing."""
import argparse
import json
import os
from pathlib import Path
import shlex
import struct
import subprocess
import sys
import tempfile
sys.dont_write_bytecode=True
import reuse
import oracle
scene=reuse.scene
ROOT,HERE=scene.ROOT,scene.HERE
require,digest,rows=reuse.require,reuse.digest,reuse.rows
STRATEGIES=('single','multi','count')


def frames(capacity):
    counts=(capacity,0,1,capacity//2,capacity+7,capacity,0,1)
    return [dict(frame=f,phase=f%2,active=n,capacity=capacity) for f,n in enumerate(counts)]


def mesh_expected(frame,strategy):
    phase,active,capacity=(frame[k] for k in ('phase','active','capacity'))
    records=b''.join(struct.pack('<IIIiI',6 if strategy=='count' or i<active else 0,1,2,
                                3 if capacity>1 and i==capacity-1 else -1,0) for i in range(capacity))
    return oracle.geometry(phase,False)[:416]+oracle.GUARD+records+struct.pack('<I',active)+oracle.GUARD


def identity_image(reference,width,height,capacity,strategy):
    """Recolor accepted coverage; preserve every depth and guard byte."""
    require(len(reference)==width*height*8+256,'wrong identity reference size')
    require(capacity in (1,64,512) and strategy in STRATEGIES,'invalid identity case')
    def color(draw,surface):
        value=2*(draw+1)+surface
        return bytes((value&255,(value>>8)&255,255,255))
    colors={oracle.BLACK:oracle.BLACK,oracle.RED:color(0,0),
            oracle.GREEN:color(0 if strategy=='single' else capacity-1,1)}
    result=bytearray(reference)
    for offset in range(64,64+width*height*4,4):
        require(reference[offset:offset+4] in colors,'unexpected reference color')
        result[offset:offset+4]=colors[reference[offset:offset+4]]
    return bytes(result)


def check_log(stdout,stderr,capacity,strategy,device,identity=False):
    require('Validation Error:' not in stdout+stderr and 'SYNC-HAZARD' not in stdout+stderr,'validation failure')
    require('Native indexed frontier drained' in stdout,'missing drain marker')
    require(rows(stdout,'DEVICE ')==[device],'wrong device')
    require(rows(stdout,'DRAW_IDENTITY_FEATURES ')==([dict(shaderDrawParameters=True)] if identity else []),
            'wrong draw-identity feature profile')
    require(rows(stdout,'FRONTIER_FRAME ')==frames(capacity),'wrong frame/count sequence')
    features=rows(stdout,'FRONTIER_FEATURES ')
    require(len(features)==1 and features[0]['multiDrawIndirect'] is True and
            features[0]['drawIndirectCount'] is True and features[0]['maxDrawIndirectCount']>=512,'missing required native features')
    memory=rows(stderr,'MEMORY_SUMMARY ')
    require(len(memory)==1 and memory[0]['allocations']==memory[0]['frees']==memory[0]['peak_count']==9
            and memory[0]['live_count']==memory[0]['live_bytes']==0,'allocation leak/count mismatch')
    commands=rows(stderr,'COMMAND_COUNTS ')
    require(len(commands)==3 and [c['phase'] for c in commands]==[0,1,2],'missing command snapshots')
    start,end,final=commands
    require(all(final[k]==end[k] for k in end if k!='phase'),'unexpected commands during teardown')
    require(start['vkQueueSubmit2']==start['vkBeginCommandBuffer']==start['vkEndCommandBuffer']==1,'wrong setup count')
    hot={k:end[k]-start[k] for k in start if k!='phase'}
    require(all(v>=0 for v in hot.values()),'counter decreased')
    for name in ('vkQueueSubmit2','vkBeginCommandBuffer','vkEndCommandBuffer','vkCmdBeginRendering','vkCmdEndRendering',
                 'vkCreateCommandPool','vkDestroyCommandPool'):
        require(hot[name]==8,f'wrong {name} count')
    require(final['vkQueueWaitIdle']==final['vkResetCommandPool']==0 and
            final['vkCreateCommandPool']==final['vkDestroyCommandPool']==9,'wrong pool/idle policy')
    wanted=(8*capacity,0,8*capacity,0) if strategy=='single' else (8,0,8*capacity,0) if strategy=='multi' else (0,8,0,8*capacity)
    require(tuple(hot[k] for k in ('vkCmdDrawIndexedIndirect2KHR','vkCmdDrawIndexedIndirectCount2KHR',
                                  'indexed_records','counted_capacity'))==wanted,'wrong actual native draw strategy')
    allocations=rows(stderr,'ALLOCATE ');require(len(allocations)==9,'missing allocation events')
    return dict(features=features[0],memory=memory[0],commands=commands,hot_commands=hot,
                allocation_shape=sorted((a['bytes'],a['type']) for a in allocations))


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check',action='store_true')
    parser.add_argument('--scale',action='store_true')
    parser.add_argument('--identity',action='store_true',help='observe native DrawIndex; intentionally different single/multi pixels')
    parser.add_argument('--reference32',type=Path)
    args=parser.parse_args()
    base=ROOT/'target/graphics-scene';base.mkdir(parents=True,exist_ok=True)
    dest=Path(tempfile.mkdtemp(prefix='frontier-',dir=base));print(f'Frontier evidence: {dest}',flush=True)
    report=dict(schema=2,complete=False,scope=__doc__,runs=[],baseline=[],scale=args.scale,identity=args.identity,
        revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        dirty=bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True)))
    def save(): (dest/'report.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
    save()
    try:
        compiler=shlex.split(os.getenv('CC','cc'))
        report['toolchain']=dict(cc=subprocess.check_output([*compiler,'--version'],text=True).splitlines()[0],
                                slang=scene.generate.VERSION,python=sys.version.split()[0])
        sources=[p for p in HERE.iterdir() if p.suffix in ('.c','.h','.py','.slang','.md')]
        sources += [ROOT/'examples/learned_image'/n for n in ('native.c','native_workload.h','extent.h','trace_memory.c','allocation_tracker.h')]
        sources += list((ROOT/'examples/learned_image/generated').glob('*.h'))+list((ROOT/'examples/compiler').glob('*.py'))
        sources += [ROOT/'include/ogpu.h',ROOT/'vendor/Vulkan-Headers/include/vulkan/vulkan_core.h']
        report['sources']={str(p.relative_to(ROOT)):digest(p) for p in sorted(sources)}
        report['shaders']={}
        raster='identity' if args.identity else 'scene'
        for name,stage in [('frontier','compute'),(raster+'.vert','vertex'),(raster+'.frag','fragment')]:
            reflected,assembly,binary=scene.generate.compile_source(HERE/(name+'.slang'),dest/stage,os.getenv('SLANGC','slangc'),stage)
            if stage!='fragment':
                parameters=reflected['parameters'];require(len(parameters)==1,'wrong root count')
                root=parameters[0]['type']['elementType']
                fields=[(f['name'],f['binding']['offset'],f['binding']['size']) for f in root['fields']]
                expected=[('vertices',0,8)]+([('indices',8,8),('draws',16,8),('control',24,8)] if stage=='compute' else [])
                require(fields==expected and scene.generate.uniform_size(root)==(32 if stage=='compute' else 8,8),'wrong root layout')
            if stage=='compute': require(reflected['entryPoints'][0]['threadGroupSize']==[8,1,1],'wrong workgroup')
            if stage=='vertex':
                require(('OpCapability DrawParameters' in assembly)==args.identity and
                        ('BuiltIn DrawIndex' in assembly)==args.identity,'unexpected draw-identity shader capability/builtin')
            (dest/(stage+'.spv')).write_bytes(binary)
            report['shaders'][stage]=dict(sha256=digest(dest/(stage+'.spv')),reflection=reflected)
        common=[*compiler,'-std=c11','-O2','-Wall','-Wextra','-Werror','-I'+str(ROOT/'include'),
                '-I'+str(ROOT/'vendor/Vulkan-Headers/include'),'-I'+str(ROOT/'examples/learned_image/generated')]
        report['builds']={}
        for name,source,extra in [('native','frontier.c',[]),('baseline','native.c',[]),('trace.so','reuse_trace.c',['-shared','-fPIC'])]:
            if name=='native' and args.identity: extra=['-DNATIVE_DRAW_IDENTITY']
            command=common+extra+[str(HERE/source),'-ldl','-o',str(dest/name)]
            subprocess.run(command,check=True)
            require('ogpu' not in subprocess.check_output(['nm','-u',str(dest/name)],text=True).lower(),'unexpected OGPU linkage')
            report['builds'][name]=dict(command=command,sha256=digest(dest/name))
        subprocess.run([sys.executable,'-B',str(HERE/'test_frontier.py')],check=True)
        if args.check:
            report.update(build_only=True,complete=True);save();print('Frontier build/parser gates PASS; no GPU');return
        require(args.reference32,'accepted UINT32 reference required')
        require(os.getenv('VK_DRIVER_FILES') and 'VK_LAYER_KHRONOS_validation' in os.getenv('VK_INSTANCE_LAYERS','')
                and os.getenv('VK_LAYER_VALIDATE_SYNC')=='1' and not os.getenv('VK_LOADER_LAYERS_DISABLE'),'select ICD and Vulkan/sync validation')
        extents=[(257,193)]+([(1280,720)] if args.scale else [])
        report['reference'],refs=reuse.reference(args.reference32,32,extents)
        report['reference_runs']=list(refs.values())
        reference_report=json.loads(args.reference32.read_text())
        for stage in ('compute','vertex','fragment'):
            for ref in refs.values():
                require(digest(Path(ref['directory']).parent/(stage+'.spv'))==reference_report['shaders'][stage]['sha256'],
                        'reference shader hash mismatch')
            if stage!='compute' and not args.identity:
                require(report['shaders'][stage]['sha256']==reference_report['shaders'][stage]['sha256'],
                        'raster shader differs from accepted reference')
        env=os.environ.copy();loader=env.get('OGPU_VULKAN_LIBRARY','libvulkan.so.1')
        env.update(OGPU_TRACE_LOADER=loader,OGPU_VULKAN_LIBRARY=str(dest/'trace.so'),OGPU_SCENE_TRACE='1')
        report['environment']={k:env.get(k) for k in ('VK_DRIVER_FILES','VK_ICD_FILENAMES','VK_INSTANCE_LAYERS',
            'VK_LAYER_VALIDATE_SYNC','OGPU_TRACE_LOADER','OGPU_VULKAN_LIBRARY','OGPU_SCENE_TRACE')}
        shapes={}
        def execute(name,command):
            out=dest/name;out.mkdir()
            with (out/'stdout').open('w') as stdout,(out/'stderr').open('w') as stderr:
                result=subprocess.run(command,stdout=stdout,stderr=stderr,env=env,timeout=300)
            require(result.returncode==0,f'execution failed: {out}')
            return out,(out/'stdout').read_text(),(out/'stderr').read_text()
        for width,height in extents:
            ref=refs[(width,height)];reference=Path(ref['directory']);device=ref['device']
            name=f'baseline-{width}x{height}'
            command=[str(dest/'baseline'),str(width),str(height),str(reference.parent),str(dest/name),'32']
            out,stdout,stderr=execute(name,command)
            require('Validation Error:' not in stdout+stderr and 'SYNC-HAZARD' not in stdout+stderr and
                    'Native indexed/depth frames drained' in stdout and rows(stdout,'DEVICE ')==[device],'baseline failed')
            require(not rows(stdout,'FRONTIER_FEATURES '),'experimental features escaped into baseline')
            require(not rows(stdout,'DRAW_IDENTITY_FEATURES '),'draw-identity profile escaped into baseline')
            wanted=[dict(mode=m,phase=int(f==1),frame=f) for m in range(10) for f in range(3)]
            require(rows(stdout,'SCENE_FRAME ')==wanted,'baseline frame mismatch')
            compared=scene.compare_outputs(reference,out,wanted)
            memory=rows(stderr,'MEMORY_SUMMARY ')
            require(len(memory)==1 and memory[0]['allocations']==memory[0]['frees']==8 and
                    memory[0]['live_count']==memory[0]['live_bytes']==0,'baseline leak')
            report['baseline'].append(dict(name=name,command=command,comparison=compared,memory=memory[0],
                                          logs={n:digest(out/n) for n in ('stdout','stderr')}));save()
            for capacity in (1,64,512):
                for strategy in STRATEGIES:
                    name=f'{width}x{height}-{capacity}-{strategy}'
                    command=[str(dest/'native'),str(width),str(height),str(dest),str(dest/name),str(capacity),strategy]
                    out,stdout,stderr=execute(name,command)
                    result=check_log(stdout,stderr,capacity,strategy,device,args.identity)
                    key=(width,height,capacity);shape=(result['allocation_shape'],result['memory']['peak_bytes'])
                    if key in shapes: require(shapes[key]==shape,'strategy memory budgets differ')
                    else: shapes[key]=shape
                    artifacts={}
                    for frame in frames(capacity):
                        f=frame['frame'];active=min(frame['active'],capacity)
                        mode=9 if active==0 else 0 if capacity>1 and active==capacity else 4
                        expected=reference/f'mode-{mode}-frame-{frame["phase"]}.images'
                        expected_bytes=expected.read_bytes()
                        if args.identity: expected_bytes=identity_image(expected_bytes,width,height,capacity,strategy)
                        require((out/f'frame-{f}.images').read_bytes()==expected_bytes,'full image/reference mismatch')
                        require((out/f'frame-{f}.geometry').read_bytes()==mesh_expected(frame,strategy),'generated records/count/guards mismatch')
                        for suffix in ('images','geometry'):
                            filename=f'frame-{f}.{suffix}';artifacts[filename]=digest(out/filename)
                    requested=width*height*16+1232+2*(132+20*capacity)+max(256,132+20*capacity)
                    report['runs'].append(dict(name=name,command=command,extent=[width,height],capacity=capacity,strategy=strategy,
                        device=device,frames=frames(capacity),requested_bytes=requested,files=artifacts,**result,
                        logs={n:digest(out/n) for n in ('stdout','stderr')}));save()
                    print(f'{name}: eight frames, generated records/count, guards and native strategy PASS',flush=True)
                if args.identity:
                    # Positive equality AND a real counterexample; do not accept a
                    # shader that drops DrawIndex or a mapping that silently fuses.
                    for f in range(8):
                        images=[(dest/f'{width}x{height}-{capacity}-{s}'/f'frame-{f}.images').read_bytes() for s in STRATEGIES]
                        require(images[1]==images[2],'multi/count identity differs')
                        differs=capacity>1 and frames(capacity)[f]['active']>=capacity
                        require((images[0]!=images[1])==differs,'missing/unexpected grouping counterexample')
        report['complete']=True;save();print('Native indexed frontier PASS; no timing/API acceptance')
    except Exception as error:
        report['error']=str(error);save();raise


if __name__=='__main__': main()
