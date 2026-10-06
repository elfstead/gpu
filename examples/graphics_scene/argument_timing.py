#!/usr/bin/env python3
"""Argument-supply timings with collection/encoding boundaries; no API parity claim."""
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
import argument_reuse as a
import range_timing as timing
ROOT,HERE=a.ROOT,a.HERE
require,digest=a.require,a.digest


def matrix(report):
    require(report.get('complete') and not report.get('build_only'),'complete executed correctness required')
    sizes=report['sizes'];require(sizes and len(set(sizes))==len(sizes) and all(n in a.snapshot.SIZES for n in sizes),'invalid size matrix')
    expected=set(itertools.product(sizes,(1,64,512),(1,2),a.PATHS))
    found=[]
    for row in report['runs']:
        found.append((row['bytes'],row['capacity'],row['slots'],row['path']))
        summary=row['summary']
        require(summary['frames']==report['frames'] and summary['replay']==0 and summary['strategy']==0
                and summary['capacity']==row['capacity'] and summary['slots']==row['slots'],'wrong recording coverage')
    require(len(found)==len(expected) and set(found)==expected,'missing/duplicate correctness case')
    return sizes


def parse(stdout,stderr,device,path,size,capacity,slots,frames):
    require(a.f.rows(stdout,'ARGUMENT_POLICY ')==[dict(bytes=size,reverse_alternating=True,resupply=a.resupply(path))],'wrong supply/order policy')
    require(not a.f.rows(stdout,'ARGUMENT_INPUT ') and not a.f.rows(stderr,'ARGUMENT_COMMANDS '),'diagnostics present during timing')
    result=timing.parse(stdout,stderr,device,capacity,'single',slots,False,frames)
    samples=a.f.rows(stdout,'SAMPLE ');parts=a.f.rows(stdout,'ARGUMENT_SAMPLE ')
    require(len(parts)==frames and [s['index'] for s in parts]==list(range(frames)),'missing/duplicate host boundary samples')
    for sample,part in zip(samples,parts):
        require(all(math.isfinite(part[k]) and part[k]>=0 for k in ('record_ms','submit_ms')),'invalid host boundary duration')
        require(part['record_ms']+part['submit_ms']<=sample['record_submit_ms']+1e-6,'host boundary intervals exceed whole sample')
    for key in ('record_ms','submit_ms'):
        values=sorted(p[key] for p in parts)
        result['statistics'][key]=dict(median=statistics.median(values),p95=values[math.ceil(.95*frames)-1],maximum=max(values))
    return result


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--correctness',required=True,type=Path)
    parser.add_argument('--preflight',action='store_true',help='16 samples/one round; development, not timing acceptance')
    args=parser.parse_args()
    out=Path(tempfile.mkdtemp(prefix='argument-timing-',dir=ROOT/'target/graphics-scene'));print(f'Argument timing evidence: {out}',flush=True)
    report=dict(schema=1,complete=False,preflight=args.preflight,scope=__doc__,runs=[],builds={},
        revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        dirty=bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True)))
    def save():(out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    save()
    try:
        correct=json.loads(args.correctness.read_text());sizes=matrix(correct);directory=args.correctness.resolve().parent
        if not args.preflight:
            require(not correct['dirty'] and not report['dirty'] and correct['frames']>=64,'clean full correctness/source required')
            for name,h in correct['sources'].items():require(digest(Path(name))==h,'source changed since correctness: '+name)
        for name,h in correct['references'].items():require(digest(directory/name)==h,'changed reference/shader')
        for case in correct['runs']:
            for name,h in case['logs'].items():require(digest(directory/name)==h,'changed correctness log')
        require(not os.getenv('CARGO_TARGET_DIR'),'leave CARGO_TARGET_DIR unset')
        subprocess.run(['cargo','build','--locked','--release','-p','ogpu'],cwd=ROOT,check=True)
        require(digest(ROOT/'target/release/libogpu.so')==correct['runtime_sha256'],'runtime differs from correctness')
        report.update(correctness=dict(path=str(args.correctness.resolve()),sha256=digest(args.correctness)),
            runtime_sha256=correct['runtime_sha256'],sources=correct['sources'].copy(),device=correct['device'],sizes=sizes)
        cc=shlex.split(os.getenv('CC','cc'));report['compiler']=subprocess.check_output([*cc,'--version'],text=True).splitlines()[0]
        for size in sizes:
            for path in a.PATHS:
                opts=a.flags(cc,size,path,timing=True)
                command=opts+[str(HERE/'range_timing.c'),'-ldl']
                if path.startswith('public'):command+=['-L'+str(ROOT/'target/release'),'-Wl,-rpath,'+str(ROOT/'target/release'),'-logpu']
                command+=['-o',str(out/f'{size}-{path}')];subprocess.run(command,check=True)
                report['builds'][f'{size}-{path}']=dict(command=command,sha256=digest(out/f'{size}-{path}'))
                deps=subprocess.check_output([*opts,'-MM',str(HERE/'range_timing.c')],text=True)
                for p in shlex.split(deps.replace('\\\n','').split(':',1)[1]):report['sources'][str(Path(p).resolve())]=digest(Path(p))
                symbols=subprocess.check_output(['nm','-u',str(out/f'{size}-{path}')],text=True)
                require('ogpu' not in symbols.lower() if not path.startswith('public') else 'ogpu_batch_submit' in symbols,'wrong timing linkage')
        for p in (Path(__file__),HERE/'range_timing.py',HERE/'argument_reuse.py'):report['sources'][str(p.resolve())]=digest(p)
        env=os.environ.copy()
        require(env.get('VK_DRIVER_FILES')==correct['environment']['VK_DRIVER_FILES'],'select correctness ICD')
        require(not env.get('LD_PRELOAD') and not env.get('OGPU_SCENE_TRACE'),'remove diagnostic injection')
        require(env.get('OGPU_VULKAN_LIBRARY','libvulkan.so.1')==correct['environment']['OGPU_TRACE_LOADER'],'use actual correctness loader')
        env.update(VK_INSTANCE_LAYERS='',VK_LAYER_VALIDATE_SYNC='0',VK_LOADER_LAYERS_DISABLE='*')
        report['environment']={k:env.get(k) for k in ('VK_DRIVER_FILES','VK_ICD_FILENAMES','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC','VK_LOADER_LAYERS_DISABLE','OGPU_VULKAN_LIBRARY')}
        frames,rounds=(16,1) if args.preflight else (992,3)
        for repeat in range(rounds):
            order=a.PATHS[repeat:]+a.PATHS[:repeat]
            ordered_sizes=sizes[repeat:]+sizes[:repeat]
            for size,capacity,slots in itertools.product(ordered_sizes,(1,64,512),(1,2)):
                for path in order:
                    name=f'{size}-{capacity}-{slots}-{path}-{repeat}';folder=out/name;folder.mkdir()
                    command=[str(out/f'{size}-{path}'),'257','193',str(directory/str(size)),str(directory/str(size)/str(capacity)),
                        str(frames),str(slots),'reset',str(capacity),'single']
                    with (folder/'stdout').open('w') as stdout,(folder/'stderr').open('w') as stderr:
                        result=subprocess.run(command,env=env,stdout=stdout,stderr=stderr,timeout=300)
                    require(result.returncode==0,'timing/final bytes failed: '+str(folder))
                    checked=parse((folder/'stdout').read_text(),(folder/'stderr').read_text(),correct['device'],path,size,capacity,slots,frames)
                    report['runs'].append(dict(name=name,bytes=size,capacity=capacity,slots=slots,path=path,repeat=repeat,
                        command=command,**checked,logs={p.name:digest(p) for p in folder.iterdir()}));save()
                print(f'{size}-byte / {capacity} draws / {slots} slots / round {repeat}: timings and final-slot bytes PASS',flush=True)
        report['complete']=True;save();print('Argument timing '+('PREFLIGHT' if args.preflight else 'COMPLETE')+'; no general performance claim')
    except Exception as error:report['error']=str(error);save();raise


if __name__=='__main__':main()
