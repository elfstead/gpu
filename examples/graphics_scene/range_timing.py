#!/usr/bin/env python3
"""Grouped range timings: matched reset/replay, GPU-copy-complete wall time, not general Vulkan parity."""
import argparse
import itertools
import json
import math
import os
from pathlib import Path
import shlex
import statistics
import subprocess
import sys
import tempfile
sys.dont_write_bytecode=True
import range_reuse as r
ROOT,HERE=r.ROOT,r.HERE
require,digest=r.require,r.digest


def parse(stdout,stderr,device,capacity,strategy,slots,replay,frames):
    require('Validation Error:' not in stdout+stderr and 'SYNC-HAZARD' not in stdout+stderr,'unexpected validation error')
    require('Range timing final-slot bytes checked and all work drained' in stdout,'missing final checks/drain')
    require(r.f.rows(stdout,'DEVICE ')==[device],'wrong timing device')
    results=r.f.rows(stdout,'RANGE_TIMING ');require(len(results)==1,'missing/duplicate result')
    result=results[0]
    wanted=dict(frames=frames,warmups=100,slots=slots,capacity=capacity,strategy=r.f.STRATEGIES.index(strategy),
                replay=int(replay),peak_unretired=slots,encodes=slots if replay else frames)
    require({k:result[k] for k in wanted}==wanted,'wrong measured schedule')
    require(all(math.isfinite(result[k]) and result[k]>0 for k in ('setup_ms','wall_ms')),'invalid window duration')
    samples=r.f.rows(stdout,'SAMPLE ')
    require(len(samples)==frames and [s['index'] for s in samples]==list(range(frames)),'missing/duplicate samples')
    metrics=('record_submit_ms','wait_ms','retirement_ms')
    for sample in samples:
        require(all(math.isfinite(sample[k]) and sample[k]>=0 for k in metrics),'invalid sample')
        require(sample['retirement_ms']+1e-6>=sample['record_submit_ms'],'retirement precedes submit')
    require(sum(s['record_submit_ms']+s['wait_ms'] for s in samples)<=result['wall_ms']+1e-3,'CPU intervals exceed wall window')
    stats={}
    for key in metrics:
        values=sorted(s[key] for s in samples)
        stats[key]=dict(median=statistics.median(values),p95=values[math.ceil(.95*frames)-1],maximum=max(values))
    return dict(result=result,statistics=stats,wall_ms_per_frame=result['wall_ms']/frames)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--correctness',required=True,type=Path,help='matched Radeon range reuse report')
    parser.add_argument('--preflight',action='store_true',help='one process/16 samples; not accepted timing')
    args=parser.parse_args()
    out=Path(tempfile.mkdtemp(prefix='range-timing-',dir=ROOT/'target/graphics-scene'));print(f'Range timing evidence: {out}',flush=True)
    report=dict(schema=1,complete=False,preflight=args.preflight,scope=__doc__,runs=[],
                revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
                dirty=bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True)))
    def save(): (out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    save()
    try:
        require(not os.getenv('CARGO_TARGET_DIR'),'leave CARGO_TARGET_DIR unset')
        correct=json.loads(args.correctness.read_text())
        require(correct.get('complete') and not correct.get('software') and len(correct['runs'])==72,'complete Radeon correctness required')
        if not args.preflight:
            require(not correct['preflight'] and not correct['dirty'] and not report['dirty'],'clean sustained correctness/source required')
            require(all(c['summary']['frames']==1000 for c in correct['runs']),'sustained matrix missing')
            for name,h in correct['sources'].items():require(digest(ROOT/name)==h,'source changed since correctness: '+name)
        report['correctness']=dict(path=str(args.correctness.resolve()),sha256=digest(args.correctness),revision=correct['revision'])
        native_path=Path(correct['native']['path']);require(digest(native_path)==correct['native']['sha256'],'changed native reference')
        native=json.loads(native_path.read_text());directory=native_path.parent
        for stage,h in correct['shaders'].items():require(digest(directory/(stage+'.spv'))==h,'changed shader')
        report['shaders']=correct['shaders']
        cases={c['name']:c for c in native['runs']}
        for case in cases.values():
            for filename,h in case['files'].items():require(digest(directory/case['name']/filename)==h,'changed oracle artifact')
        subprocess.run(['cargo','build','--locked','--release','-p','ogpu'],cwd=ROOT,check=True)
        require(digest(ROOT/'target/release/libogpu.so')==correct['runtime_sha256'],'runtime differs from correctness')
        report['runtime_sha256']=correct['runtime_sha256'];report['sources']=correct['sources'].copy()
        for name in ('range_timing.c','range_timing.py'):report['sources']['examples/graphics_scene/'+name]=digest(HERE/name)
        cc=shlex.split(os.getenv('CC','cc'));report['compiler']=subprocess.check_output([*cc,'--version'],text=True).splitlines()[0]
        report['builds']={}
        subprocess.run([sys.executable,'-B',str(HERE/'test_range_timing.py')],check=True)
        for backend in ('native','public'):
            command=[*cc,'-std=c11','-O2','-Wall','-Wextra','-Werror','-I'+str(ROOT/'include'),str(HERE/'range_timing.c')]
            if backend=='native':command+=['-DRANGE_NATIVE','-I'+str(ROOT/'vendor/Vulkan-Headers/include'),'-I'+str(ROOT/'examples/learned_image/generated')]
            else:command+=['-L'+str(ROOT/'target/release'),'-Wl,-rpath,'+str(ROOT/'target/release'),'-logpu']
            command+=['-ldl','-o',str(out/backend)];subprocess.run(command,check=True)
            symbols=subprocess.check_output(['nm','-u',str(out/backend)],text=True)
            require('ogpu' not in symbols.lower() if backend=='native' else 'ogpu_command_list_submit' in symbols,'wrong linkage')
            report['builds'][backend]=dict(command=command,sha256=digest(out/backend))
        env=os.environ.copy()
        require(env.get('VK_DRIVER_FILES')==correct['environment']['VK_DRIVER_FILES'],'select correctness ICD')
        require(not env.get('LD_PRELOAD') and not env.get('OGPU_SCENE_TRACE'),'remove diagnostic injection')
        require(env.get('OGPU_VULKAN_LIBRARY','libvulkan.so.1')==correct['environment']['OGPU_TRACE_LOADER'],'use actual correctness loader, not tracing shim')
        env.update(VK_INSTANCE_LAYERS='',VK_LAYER_VALIDATE_SYNC='0',VK_LOADER_LAYERS_DISABLE='*')
        report['environment']={k:env.get(k) for k in ('VK_DRIVER_FILES','VK_ICD_FILENAMES','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC','VK_LOADER_LAYERS_DISABLE','OGPU_VULKAN_LIBRARY')}
        frames,rounds=(16,1) if args.preflight else (1000,3)
        for repeat in range(rounds):
            strategies=r.f.STRATEGIES[repeat:]+r.f.STRATEGIES[:repeat]
            backends=('native','public') if repeat%2==0 else ('public','native')
            for capacity,slots,policy,strategy in itertools.product((1,64,512),(1,2),('reset','replay'),strategies):
                reference=cases[f'257x193-{capacity}-{strategy}']
                for backend in backends:
                    name=f'{capacity}-{strategy}-{slots}-{policy}-{backend}-{repeat}';dest=out/name;dest.mkdir()
                    command=[str(out/backend),'257','193',str(directory),str(directory/reference['name']),str(frames),str(slots),policy,str(capacity),strategy]
                    with (dest/'stdout').open('w') as stdout,(dest/'stderr').open('w') as stderr:
                        execution=subprocess.run(command,env=env,stdout=stdout,stderr=stderr,timeout=300)
                    require(execution.returncode==0,f'timing/final verification failed: {dest}')
                    checked=parse((dest/'stdout').read_text(),(dest/'stderr').read_text(),reference['device'],capacity,strategy,slots,policy=='replay',frames)
                    report['runs'].append(dict(name=name,backend=backend,capacity=capacity,strategy=strategy,slots=slots,policy=policy,repeat=repeat,
                        device=reference['device'],command=command,**checked,logs={n:digest(dest/n) for n in ('stdout','stderr')}));save()
                print(name.rsplit('-',2)[0]+f' round={repeat}: measurements and final bytes PASS',flush=True)
        report['complete']=True;save();print('Grouped range timing '+('PREFLIGHT' if args.preflight else 'COMPLETE')+'; no general performance claim')
    except Exception as error:report['error']=str(error);save();raise


if __name__=='__main__':main()
