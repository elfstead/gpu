#!/usr/bin/env python3
"""ABI-20 public fixed/count/identity scene against accepted native identity controls."""
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


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native',type=Path)
    parser.add_argument('--check',action='store_true')
    args=parser.parse_args()
    base=ROOT/'target/graphics-scene';base.mkdir(parents=True,exist_ok=True)
    out=Path(tempfile.mkdtemp(prefix='ranges-',dir=base));print(f'Public range evidence: {out}',flush=True)
    report=dict(schema=1,abi=20,complete=False,scope=__doc__,runs=[],
        revision=subprocess.check_output(['git','rev-parse','HEAD'],text=True,cwd=ROOT).strip(),
        dirty=bool(subprocess.check_output(['git','status','--porcelain'],text=True,cwd=ROOT)))
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
            ('public',common+[str(HERE/'frontier_public.c'),'-L'+str(ROOT/'target/release'),'-Wl,-rpath,'+str(ROOT/'target/release'),'-logpu','-ldl','-o',str(out/'public')]),
            ('trace.so',common+['-shared','-fPIC','-I'+str(ROOT/'vendor/Vulkan-Headers/include'),str(HERE/'reuse_trace.c'),'-ldl','-o',str(out/'trace.so')])]:
            subprocess.run(command,check=True);report['builds'][name]=dict(command=command,sha256=digest(out/name))
        symbols=subprocess.check_output(['nm','-u',str(out/'public')],text=True)
        require('ogpu_batch_draw_indexed_indirect' in symbols and not any(line.split()[-1].startswith('vk') for line in symbols.splitlines() if line.split()),'wrong public linkage')
        sources=list(HERE.glob('*.c'))+list(HERE.glob('*.h'))+list(HERE.glob('*.py'))+[ROOT/'include/ogpu.h']
        sources+=list((ROOT/'crates/ogpu/src').glob('*.rs'))+[ROOT/'crates/vulkan-sys/src/bindings.rs']
        sources+=[ROOT/'examples/learned_image'/name for name in ('trace_memory.c','allocation_tracker.h')]
        report['sources']={str(p.relative_to(ROOT)):digest(p) for p in sorted(sources)}
        report['runtime_sha256']=digest(ROOT/'target/release/libogpu.so')
        subprocess.run([sys.executable,'-B',str(HERE/'test_frontier.py')],check=True)
        if args.check: report.update(complete=True,build_only=True);save();return
        require(args.native,'accepted native identity report required')
        native=json.loads(args.native.read_text());directory=args.native.parent.resolve()
        require(native.get('complete') and not native.get('dirty') and native.get('identity') and not native.get('build_only'),'reference must be a clean native identity execution')
        require(len(native['runs'])==9 and all(c['extent']==[257,193] for c in native['runs']),'this checkpoint expects the nine small native cases')
        require(digest(Path(native['reference']['path']))==native['reference']['sha256'],'serial reference hash mismatch')
        _,refs=f.reuse.reference(Path(native['reference']['path']),32,[(257,193)])
        reference=Path(refs[(257,193)]['directory'])
        report['native']=dict(path=str(args.native.resolve()),sha256=digest(args.native),revision=native['revision'])
        report['shaders']={}
        for stage,metadata in native['shaders'].items():
            require(digest(directory/(stage+'.spv'))==metadata['sha256'],'native shader hash mismatch')
            report['shaders'][stage]=metadata['sha256']
        env=os.environ.copy()
        require(env.get('VK_DRIVER_FILES') and 'VK_LAYER_KHRONOS_validation' in env.get('VK_INSTANCE_LAYERS','') and env.get('VK_LAYER_VALIDATE_SYNC')=='1'
                and not env.get('VK_LOADER_LAYERS_DISABLE'),'select one ICD with validation/sync enabled')
        env.update(OGPU_TRACE_LOADER=env.get('OGPU_VULKAN_LIBRARY','libvulkan.so.1'),OGPU_VULKAN_LIBRARY=str(out/'trace.so'),OGPU_SCENE_TRACE='1')
        report['environment']={k:env.get(k) for k in ('VK_DRIVER_FILES','VK_ICD_FILENAMES','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC','OGPU_TRACE_LOADER','OGPU_VULKAN_LIBRARY')}
        for case in native['runs']:
            name=case['name'];dest=out/name;dest.mkdir();capacity=case['capacity'];strategy=case['strategy']
            for filename,h in {**case['files'],**case['logs']}.items():require(digest(directory/name/filename)==h,'native artifact hash mismatch')
            f.check_log((directory/name/'stdout').read_text(),(directory/name/'stderr').read_text(),capacity,strategy,case['device'],True)
            command=[str(out/'public'),'257','193',str(directory),str(dest),str(capacity),strategy]
            with (dest/'stdout').open('w') as stdout,(dest/'stderr').open('w') as stderr:
                execution=subprocess.run(command,env=env,stdout=stdout,stderr=stderr,timeout=300)
            require(execution.returncode==0,f'public execution failed: {dest}')
            stdout=(dest/'stdout').read_text();stderr=(dest/'stderr').read_text()
            require(f.rows(stdout,'PUBLIC_SCENE ')==[dict(abi=20,index_bytes=4,gpu_generated=True)],'wrong public ABI/profile')
            checked=f.check_log(stdout,stderr,capacity,strategy,case['device'],True,True)
            require([list(a) for a in checked['allocation_shape']]==case['allocation_shape'] and checked['memory']['peak_bytes']==case['memory']['peak_bytes'],'allocation budgets differ')
            for frame in f.frames(capacity):
                index=frame['frame'];active=min(frame['active'],capacity)
                mode=9 if active==0 else 0 if capacity>1 and active==capacity else 4
                expected=f.identity_image((reference/f'mode-{mode}-frame-{frame["phase"]}.images').read_bytes(),257,193,capacity,strategy)
                require((dest/f'frame-{index}.images').read_bytes()==expected,'identity oracle mismatch')
                require((dest/f'frame-{index}.geometry').read_bytes()==f.mesh_expected(frame,strategy),'geometry/count oracle mismatch')
            for filename,h in case['files'].items():require(digest(dest/filename)==h,'public/native output mismatch')
            report['runs'].append(dict(name=name,command=command,capacity=capacity,strategy=strategy,device=case['device'],
                requested_bytes=case['requested_bytes'],files=case['files'],**checked,logs={n:digest(dest/n) for n in ('stdout','stderr')}));save()
            print(name+': public/native byte equality, identity, count and allocation gates PASS',flush=True)
        report['complete']=True;save();print('Public indirect ranges PASS; timing and wider acceptance remain')
    except Exception as error: report['error']=str(error);save();raise


if __name__=='__main__': main()
