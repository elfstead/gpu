#!/usr/bin/env python3
"""Public fixed/count/identity scene against accepted native identity controls."""
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


def inspect_shaders(directory,out):
    interfaces={}
    for name,stage in [('frontier','compute'),('identity.vert','vertex'),('identity.frag','fragment')]:
        reflected,assembly,binary=f.scene.generate.compile_source(HERE/(name+'.slang'),out/('inspect-'+stage),os.getenv('SLANGC','slangc'),stage)
        require(binary==(directory/(stage+'.spv')).read_bytes(),'compiler/native shader bytes differ')
        interfaces[stage]=f.scene.inspect_foundation_shader(reflected,assembly,stage,draw_identity=True)
        if stage!='fragment':
            root=reflected['parameters'][0]['type']['elementType']
            fields=[(field['name'],field['binding']['offset'],field['binding']['size']) for field in root['fields']]
            require(fields==[('vertices',0,8)]+([('indices',8,8),('draws',16,8),('control',24,8)] if stage=='compute' else []),'wrong foundation root fields')
    f.scene.generate.stages.link(interfaces['vertex'],interfaces['fragment'])


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native',type=Path)
    parser.add_argument('--check',action='store_true')
    parser.add_argument('--foundation',action='store_true',help='explicit foundation ownership, no ABI20 runtime policy')
    parser.add_argument('--scale',action='store_true',help='also 1280x720; requires a matching scaled native acceptance')
    args=parser.parse_args()
    base=ROOT/'target/graphics-scene';base.mkdir(parents=True,exist_ok=True)
    out=Path(tempfile.mkdtemp(prefix='ranges-',dir=base));print(f'Public range evidence: {out}',flush=True)
    report=dict(schema=1,abi=20,complete=False,scope=__doc__,runs=[],
        revision=subprocess.check_output(['git','rev-parse','HEAD'],text=True,cwd=ROOT).strip(),
        dirty=bool(subprocess.check_output(['git','status','--porcelain'],text=True,cwd=ROOT)))
    report['api']='foundation' if args.foundation else 'installed'
    report['scale']=args.scale
    if args.foundation: del report['abi'];report['foundation']=1
    def save(): (out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    save()
    try:
        require(not os.getenv('CARGO_TARGET_DIR'),'leave CARGO_TARGET_DIR unset')
        subprocess.run(['cargo','build','--locked','--release','-p','ogpu'],cwd=ROOT,check=True)
        cc=shlex.split(os.getenv('CC','cc'));report['cc']=subprocess.check_output([*cc,'--version'],text=True).splitlines()[0]
        includes=['-I'+str(ROOT/'include')]
        common=[*cc,'-std=c11','-O2','-Wall','-Wextra','-Werror',*includes]
        report['builds']={}
        for name,command in [
            ('public',common+[str(HERE/('frontier_foundation.c' if args.foundation else 'frontier_public.c')),'-L'+str(ROOT/'target/release'),'-Wl,-rpath,'+str(ROOT/'target/release'),'-logpu','-ldl','-o',str(out/'public')]),
            ('trace.so',common+['-shared','-fPIC','-I'+str(ROOT/'vendor/Vulkan-Headers/include'),str(HERE/'reuse_trace.c'),'-ldl','-o',str(out/'trace.so')])]:
            subprocess.run(command,check=True);report['builds'][name]=dict(command=command,sha256=digest(out/name))
        symbols=subprocess.check_output(['nm','-u',str(out/'public')],text=True)
        require(('ogpu_next_draw_indexed_indirect' if args.foundation else 'ogpu_batch_draw_indexed_indirect') in symbols and not any(line.split()[-1].startswith('vk') for line in symbols.splitlines() if line.split()),'wrong public linkage')
        if args.foundation:
            require(all(line.split()[-1].startswith('ogpu_next_') for line in symbols.splitlines() if line.split() and line.split()[-1].startswith('ogpu_')),'foundation imports legacy runtime')
            command=[str(HERE/'foundation_failures.c') if word==str(HERE/'frontier_foundation.c') else
                     str(out/'failures') if word==str(out/'public') else word for word in report['builds']['public']['command']]
            command+=['-DSCENE_FOUNDATION_FRONTIER_FAILURES']
            command+=['-Wl,--wrap=ogpu_next_'+name for name in ('commands_end','queue_submit','timeline_wait','arena_destroy','memory_destroy','device_destroy')]
            subprocess.run(command,check=True)
            report['builds']['failures']=dict(command=command,sha256=digest(out/'failures'))
        sources=list(HERE.glob('*.c'))+list(HERE.glob('*.h'))+list(HERE.glob('*.py'))+list(HERE.glob('*.slang'))+[ROOT/'include'/('ogpu_next.h' if args.foundation else 'ogpu.h')]
        sources+=list((ROOT/'examples/compiler').glob('*.py'))
        sources+=list((ROOT/'crates/ogpu/src').glob('*.rs'))+[ROOT/'crates/vulkan-sys/src/bindings.rs']
        sources+=[ROOT/'examples/learned_image'/name for name in ('trace_memory.c','allocation_tracker.h')]
        report['sources']={str(p.relative_to(ROOT)):digest(p) for p in sorted(sources)}
        report['runtime_sha256']=digest(ROOT/'target/release/libogpu.so')
        subprocess.run([sys.executable,'-B',str(HERE/'test_frontier.py')],check=True)
        if args.check: report.update(complete=True,build_only=True);save();return
        require(args.native,'accepted native identity report required')
        native=json.loads(args.native.read_text());directory=args.native.parent.resolve()
        require(native.get('complete') and not native.get('dirty') and native.get('identity') and not native.get('build_only'),'reference must be a clean native identity execution')
        extents=[(257,193)]+([(1280,720)] if args.scale else [])
        keys=[(tuple(c['extent']),c['capacity'],c['strategy']) for c in native['runs']]
        expected={(extent,capacity,strategy) for extent in extents for capacity in (1,64,512) for strategy in f.STRATEGIES}
        require(len(keys)==len(expected) and set(keys)==expected,'complete unique native extent/capacity/strategy matrix required')
        require(digest(Path(native['reference']['path']))==native['reference']['sha256'],'serial reference hash mismatch')
        _,refs=f.reuse.reference(Path(native['reference']['path']),32,extents)
        report['native']=dict(path=str(args.native.resolve()),sha256=digest(args.native),revision=native['revision'])
        report['shaders']={}
        for stage,metadata in native['shaders'].items():
            require(digest(directory/(stage+'.spv'))==metadata['sha256'],'native shader hash mismatch')
            report['shaders'][stage]=metadata['sha256']
        if args.foundation:inspect_shaders(directory,out)
        env=os.environ.copy()
        require(env.get('VK_DRIVER_FILES') and 'VK_LAYER_KHRONOS_validation' in env.get('VK_INSTANCE_LAYERS','') and env.get('VK_LAYER_VALIDATE_SYNC')=='1'
                and not env.get('VK_LOADER_LAYERS_DISABLE'),'select one ICD with validation/sync enabled')
        env.update(OGPU_TRACE_LOADER=env.get('OGPU_VULKAN_LIBRARY','libvulkan.so.1'),OGPU_VULKAN_LIBRARY=str(out/'trace.so'),OGPU_SCENE_TRACE='1')
        report['environment']={k:env.get(k) for k in ('VK_DRIVER_FILES','VK_ICD_FILENAMES','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC','OGPU_TRACE_LOADER','OGPU_VULKAN_LIBRARY')}
        if args.foundation:
            report['failures']=[]
            for fault in ('end','submit','unknown','wait'):
                dest=out/('failure-'+fault);dest.mkdir()
                command=[str(out/'failures'),fault,'257','193',str(directory),str(dest),'64','count']
                failed=subprocess.run(command,env=env,capture_output=True,text=True,timeout=60)
                (dest/'stdout').write_text(failed.stdout);(dest/'stderr').write_text(failed.stderr)
                f.scene.check_failure(fault,failed.returncode,failed.stdout,failed.stderr,9,'FRONTIER_FRAME ')
                report['failures'].append(dict(fault=fault,command=command,returncode=failed.returncode,
                    logs={p.name:digest(p) for p in dest.iterdir()}));save()
            print('Foundation frontier rejected-record/submit cleanup and unknown-completion fail-stop PASS',flush=True)
        shapes={}
        for case in native['runs']:
            name=case['name'];dest=out/name;dest.mkdir();capacity=case['capacity'];strategy=case['strategy']
            width,height=case['extent'];reference=Path(refs[(width,height)]['directory'])
            for filename,h in {**case['files'],**case['logs']}.items():require(digest(directory/name/filename)==h,'native artifact hash mismatch')
            f.check_log((directory/name/'stdout').read_text(),(directory/name/'stderr').read_text(),capacity,strategy,case['device'],True)
            command=[str(out/'public'),str(width),str(height),str(directory),str(dest),str(capacity),strategy]
            with (dest/'stdout').open('w') as stdout,(dest/'stderr').open('w') as stderr:
                execution=subprocess.run(command,env=env,stdout=stdout,stderr=stderr,timeout=300)
            require(execution.returncode==0,f'public execution failed: {dest}')
            stdout=(dest/'stdout').read_text();stderr=(dest/'stderr').read_text()
            profile={'foundation':1} if args.foundation else {'abi':20}
            require(f.rows(stdout,'PUBLIC_SCENE ')==[dict(**profile,index_bytes=4,gpu_generated=True)],'wrong public API/profile')
            checked=f.check_log(stdout,stderr,capacity,strategy,case['device'],True,True,args.foundation)
            if args.foundation:
                requested=width*height*16+1232+2*(132+20*capacity)+max(256,132+20*capacity)
                require(case['requested_bytes']==requested and f.rows(stdout,'FRONTIER_STORAGE ')==[dict(requested_bytes=requested,arenas=1)],'wrong explicit storage budget')
                key=(width,height,capacity);shape=(checked['allocation_shape'],checked['memory']['peak_bytes'])
                if key in shapes:require(shapes[key]==shape,'foundation strategy allocation shapes differ')
                else:shapes[key]=shape
            if not args.foundation:
                require([list(a) for a in checked['allocation_shape']]==case['allocation_shape'] and checked['memory']['peak_bytes']==case['memory']['peak_bytes'],'allocation budgets differ')
            for frame in f.frames(capacity):
                index=frame['frame'];active=min(frame['active'],capacity)
                mode=9 if active==0 else 0 if capacity>1 and active==capacity else 4
                expected=f.identity_image((reference/f'mode-{mode}-frame-{frame["phase"]}.images').read_bytes(),width,height,capacity,strategy)
                require((dest/f'frame-{index}.images').read_bytes()==expected,'identity oracle mismatch')
                require((dest/f'frame-{index}.geometry').read_bytes()==f.mesh_expected(frame,strategy),'geometry/count oracle mismatch')
            for filename,h in case['files'].items():require(digest(dest/filename)==h,'public/native output mismatch')
            report['runs'].append(dict(name=name,command=command,extent=[width,height],capacity=capacity,strategy=strategy,device=case['device'],
                requested_bytes=case['requested_bytes'],files=case['files'],**checked,logs={n:digest(dest/n) for n in ('stdout','stderr')}));save()
            print(name+': public/native byte equality, identity, count and allocation lifetime gates PASS',flush=True)
        report['complete']=True;save();print('Public indirect ranges PASS; timing and wider acceptance remain')
    except Exception as error: report['error']=str(error);save();raise


if __name__=='__main__': main()
