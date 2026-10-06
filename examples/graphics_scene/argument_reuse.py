#!/usr/bin/env python3
"""Matched bind-once/resupply/public root controls with changing draw order. No timing."""
import argparse
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
sys.dont_write_bytecode=True
import argument_snapshot as snapshot
import frontier as f
import range_reuse as reuse
ROOT,HERE=f.ROOT,f.HERE
require,digest=f.require,f.digest
PATHS=('native-once','native-resupply','public')


def recolor(data,size):
    require(len(data)==snapshot.IMAGE_BYTES,'wrong reference extent')
    result=bytearray(data)
    colors={f.oracle.BLACK:f.oracle.BLACK,bytes((2,0,255,255)):snapshot.color(size,0,0),
            bytes((3,0,255,255)):snapshot.color(size,1,0)}
    for offset in range(64,64+257*193*4,4):
        require(data[offset:offset+4] in colors,'unexpected separate-call identity')
        result[offset:offset+4]=colors[data[offset:offset+4]]
    return bytes(result)


def flags(cc,size,path,diagnostic=False,timing=False):
    result=[*cc,'-std=c11','-O2','-Wall','-Wextra','-Werror',
        '-DSCENE_ARGUMENT_REUSE','-DSCENE_ARGUMENT_ROOT',f'-DSCENE_RASTER_BYTES={size}','-I'+str(ROOT/'include')]
    if not timing:result+=['-DSCENE_RANGE_REUSE']
    else:result+=['-DSCENE_ARGUMENT_PROFILE']
    if diagnostic:result+=['-DSCENE_ARGUMENT_DIAGNOSTICS']
    if path!='native-once':result+=['-DSCENE_ARGUMENT_RESUPPLY']
    if path!='public':
        result+=['-I'+str(ROOT/'vendor/Vulkan-Headers/include'),'-I'+str(ROOT/'examples/learned_image/generated')]
        if not timing:result+=['-DNATIVE_DRAW_IDENTITY']
        if timing:result+=['-DRANGE_NATIVE']
    return result


def check_arguments(stdout,stderr,path,size,capacity,frames):
    require(path in PATHS and size in snapshot.SIZES and capacity in (1,64,512)
            and frames>=16 and frames%16==0,'invalid argument control')
    require(f.rows(stdout,'ARGUMENT_POLICY ')==[dict(bytes=size,reverse_alternating=True,resupply=path!='native-once')],'wrong argument policy')
    supplied=frames*(1 if path=='native-once' else capacity)
    require(f.rows(stdout,'ARGUMENT_INPUT ')==[dict(calls=supplied,bytes=supplied*size)],'wrong supplied root accounting')
    pushes=capacity if path=='native-resupply' else 1
    trace=f.rows(stderr,'ARGUMENT_COMMANDS ')
    expected=[dict(phase=0,push_bytes=0,forward=0,backward=0,other=0)]+[
        dict(phase=phase,push_bytes=frames*(32+pushes*size),forward=frames//2*(capacity-1),
             backward=frames//2*(capacity-1),other=0) for phase in (1,2)]
    require(trace==expected,'native push bytes/order do not match changing recordings')
    return dict(argument_input=dict(calls=supplied,bytes=supplied*size),argument_commands=trace)


def check(stdout,stderr,path,size,capacity,slots,frames,device):
    arguments=check_arguments(stdout,stderr,path,size,capacity,frames)
    pushes=capacity if path=='native-resupply' else 1
    result=reuse.check(stdout,stderr,'public' if path=='public' else 'native',capacity,'single',slots,False,frames,device,root_pushes=pushes)
    result.update(arguments)
    return result


def prepare(native_path,snapshot_path,out):
    native=json.loads(native_path.read_text());snap=json.loads(snapshot_path.read_text())
    require(native.get('complete') and native.get('identity') and not native.get('dirty') and not native.get('build_only'),'clean native identity reference required')
    require(snap.get('complete') and not snap.get('dirty') and not snap.get('build_only'),'clean snapshot reference required')
    require(snap['sources'][str(HERE/'argument.vert.slang')]==digest(HERE/'argument.vert.slang'),'changed argument shader source')
    _,serials=f.reuse.reference(Path(native['reference']['path']),32,[(257,193)])
    require(digest(Path(native['reference']['path']))==native['reference']['sha256'],'changed serial reference report')
    source=native_path.resolve().parent;cases={}
    for case in native['runs']:
        if case['extent']!=[257,193] or case['strategy']!='single':continue
        require(case['capacity'] not in cases,'duplicate native capacity')
        cases[case['capacity']]=case
        for name,h in {**case['files'],**case['logs']}.items():require(digest(source/case['name']/name)==h,'changed native reference file')
    require(set(cases)=={1,64,512},'missing native capacity')
    device=serials[(257,193)]['device'];limits=[]
    for run in snap['runs']:
        require(run['device']==device,'snapshot/reference device mismatch');limits.append(run['limit']['max_push_data_bytes'])
    require(len(limits)>=2 and len(set(limits))==1,'inconsistent snapshot limits')
    sizes=[n for n in snapshot.SIZES if n<=limits[0]]
    require({(r['bytes'],r['backend']) for r in snap['runs']}=={(n,b) for n in sizes for b in ('native','public')},'incomplete supported snapshot matrix')
    serial=Path(serials[(257,193)]['directory'])
    for capacity,case in cases.items():
        for frame in f.frames(capacity):
            active=min(frame['active'],capacity)
            mode=9 if active==0 else 0 if capacity>1 and active==capacity else 4
            original=(source/case['name']/f'frame-{frame["frame"]}.images').read_bytes()
            expected=f.identity_image((serial/f'mode-{mode}-frame-{frame["phase"]}.images').read_bytes(),257,193,capacity,'single')
            require(original==expected,'native reference analytic mismatch')
            mesh=(source/case['name']/f'frame-{frame["frame"]}.geometry').read_bytes()
            require(mesh==f.mesh_expected(frame,'single'),'native reference geometry mismatch')
            for size in sizes:
                folder=out/str(size)/str(capacity);folder.mkdir(parents=True,exist_ok=True)
                (folder/f'frame-{frame["frame"]}.images').write_bytes(recolor(original,size))
                (folder/f'frame-{frame["frame"]}.geometry').write_bytes(mesh)
    for size in sizes:
        for stage in ('compute','vertex','fragment'):
            path=(source if stage=='compute' else snapshot_path.resolve().parent/str(size))/(stage+'.spv')
            wanted=(native['shaders'] if stage=='compute' else snap['shaders'][str(size)])[stage]['sha256']
            require(digest(path)==wanted,'changed shader artifact');(out/str(size)/(stage+'.spv')).write_bytes(path.read_bytes())
    return device,sizes,limits[0]


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native',required=True,type=Path)
    parser.add_argument('--snapshots',required=True,type=Path)
    parser.add_argument('--frames',type=int,choices=(16,64,992),default=64)
    parser.add_argument('--build-only',action='store_true')
    args=parser.parse_args();require(args.frames%16==0,'frames must complete two-slot eight-generation cycles')
    out=Path(tempfile.mkdtemp(prefix='argument-reuse-',dir=ROOT/'target/graphics-scene'));print(f'Argument reuse evidence: {out}',flush=True)
    report=dict(schema=1,complete=False,build_only=args.build_only,frames=args.frames,scope=__doc__,runs=[],builds={},
        revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        dirty=bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True)))
    def save():(out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    save()
    try:
        device,sizes,limit=prepare(args.native,args.snapshots,out)
        report.update(device=device,sizes=sizes,limit=limit,unsupported=[n for n in snapshot.SIZES if n not in sizes],
            native=dict(path=str(args.native.resolve()),sha256=digest(args.native)),
            snapshots=dict(path=str(args.snapshots.resolve()),sha256=digest(args.snapshots)))
        require(not os.getenv('CARGO_TARGET_DIR'),'leave CARGO_TARGET_DIR unset')
        subprocess.run(['cargo','build','--locked','--release','-p','ogpu'],cwd=ROOT,check=True)
        report['runtime_sha256']=digest(ROOT/'target/release/libogpu.so')
        cc=shlex.split(os.getenv('CC','cc'));report['compiler']=subprocess.check_output([*cc,'--version'],text=True).splitlines()[0]
        report['sources']={}
        for size in sizes:
            for path in PATHS:
                opts=flags(cc,size,path,diagnostic=True);source=HERE/('reuse_public.c' if path=='public' else 'reuse_native.c')
                command=opts+[str(source),'-ldl']
                if path=='public':command+=['-L'+str(ROOT/'target/release'),'-Wl,-rpath,'+str(ROOT/'target/release'),'-logpu']
                command+=['-o',str(out/str(size)/path)];subprocess.run(command,check=True)
                deps=subprocess.check_output([*opts,'-MM',str(source)],text=True)
                for p in shlex.split(deps.replace('\\\n','').split(':',1)[1]):report['sources'][str(Path(p).resolve())]=digest(Path(p))
                symbols=subprocess.check_output(['nm','-u',str(out/str(size)/path)],text=True)
                require(('ogpu' not in symbols.lower()) if path!='public' else 'ogpu_batch_draw_indexed_indirect' in symbols,'wrong linkage')
                report['builds'][f'{size}-{path}']=dict(command=command,sha256=digest(out/str(size)/path))
        command=[*cc,'-std=c11','-O2','-Wall','-Wextra','-Werror','-DSCENE_ARGUMENT_TRACE','-shared','-fPIC',
            '-I'+str(ROOT/'vendor/Vulkan-Headers/include'),str(HERE/'reuse_trace.c'),'-ldl','-o',str(out/'trace.so')]
        subprocess.run(command,check=True);report['builds']['trace']=dict(command=command,sha256=digest(out/'trace.so'))
        deps=subprocess.check_output([*cc,'-DSCENE_ARGUMENT_TRACE','-I'+str(ROOT/'vendor/Vulkan-Headers/include'),
                                      '-MM',str(HERE/'reuse_trace.c')],text=True)
        for p in shlex.split(deps.replace('\\\n','').split(':',1)[1]):report['sources'][str(Path(p).resolve())]=digest(Path(p))
        for p in (Path(__file__),HERE/'range_reuse.py',HERE/'argument_snapshot.py',HERE/'reuse_trace.c',
                  HERE/'frontier.py',HERE/'reuse.py',HERE/'oracle.py'):
            report['sources'][str(p.resolve())]=digest(p)
        report['references']={str(p.relative_to(out)):digest(p) for size in sizes for p in (out/str(size)).rglob('*') if p.suffix in ('.spv','.images','.geometry')}
        save()
        if not args.build_only:
            env=os.environ.copy()
            require(env.get('VK_DRIVER_FILES') and 'VK_LAYER_KHRONOS_validation' in env.get('VK_INSTANCE_LAYERS','')
                    and env.get('VK_LAYER_VALIDATE_SYNC')=='1' and not env.get('VK_LOADER_LAYERS_DISABLE'),'select ICD and validation/sync')
            env.update(OGPU_TRACE_LOADER=env.get('OGPU_VULKAN_LIBRARY','libvulkan.so.1'),OGPU_VULKAN_LIBRARY=str(out/'trace.so'),OGPU_SCENE_TRACE='1')
            report['environment']={k:env.get(k) for k in ('VK_DRIVER_FILES','VK_ICD_FILENAMES','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC','OGPU_TRACE_LOADER')}
            for size in sizes:
                for capacity in (1,64,512):
                    for slots in (1,2):
                        matched=[]
                        for path in PATHS:
                            folder=out/f'{size}-{capacity}-{slots}-{path}';folder.mkdir()
                            command=[str(out/str(size)/path),'257','193',str(out/str(size)),str(out/str(size)/str(capacity)),
                                str(args.frames),str(slots),'reset',str(capacity),'single']
                            with (folder/'stdout').open('w') as stdout,(folder/'stderr').open('w') as stderr:
                                result=subprocess.run(command,stdout=stdout,stderr=stderr,env=env,timeout=300)
                            require(result.returncode==0,'argument execution failed: '+str(folder))
                            checked=check((folder/'stdout').read_text(),(folder/'stderr').read_text(),path,size,capacity,slots,args.frames,device)
                            matched.append(checked)
                            report['runs'].append(dict(bytes=size,capacity=capacity,slots=slots,path=path,command=command,**checked,
                                logs={str(p.relative_to(out)):digest(p) for p in folder.iterdir()}));save()
                        for value in matched[1:]:
                            require(value['allocation_shape']==matched[0]['allocation_shape'] and value['memory']==matched[0]['memory'],
                                    'allocation budget/lifetime mismatch')
                        print(f'{size}-byte / {capacity} draws / {slots} slots: full outputs, allocation budgets, supplied bytes and native pushes/order PASS',flush=True)
        report['complete']=True;save();print('Argument reuse '+('BUILD ONLY' if args.build_only else 'PASS; timing/contract selection remains open'))
    except Exception as error:report['error']=str(error);save();raise


if __name__=='__main__':main()
