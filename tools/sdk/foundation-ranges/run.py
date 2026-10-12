#!/usr/bin/env python3
"""Standalone generated range/count/identity and caller-owned replay acceptance."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
sys.dont_write_bytecode=True
import scene_checks as scene
import range_oracle as oracle

HERE=Path(__file__).resolve().parent
require,digest,rows=scene.require,scene.digest,scene.rows


def profile(stdout,stderr,device,capacity):
    require('Validation Error:' not in stdout+stderr and 'SYNC-HAZARD' not in stdout+stderr,'validation failure')
    require(rows(stdout,'DEVICE ')==[device],'range device mismatch')
    require(rows(stdout,'PUBLIC_SCENE ')==[dict(foundation=1,index_bytes=4,gpu_generated=True)],'wrong foundation profile')
    require(rows(stdout,'GENERATED_SCENE ')==[dict(compute_push_bytes=32,vertex_push_bytes=8)],'wrong generated range profile')
    require(rows(stdout,'DRAW_IDENTITY_FEATURES ')==[dict(shaderDrawParameters=True)],'missing draw identity')
    features=rows(stdout,'FRONTIER_FEATURES ')
    require(len(features)==1 and features[0].get('multiDrawIndirect') is True and features[0].get('drawIndirectCount') is True
            and features[0].get('maxDrawIndirectCount',0)>=capacity+7,'insufficient range/count features')


def requested_bytes(width,height,capacity):
    draws=132+20*capacity
    return width*height*16+1232+2*draws+max(256,draws)


def check_fresh(stdout,stderr,device,width,height,capacity):
    profile(stdout,stderr,device,capacity)
    require('Public indexed frontier drained' in stdout,'missing fresh drain')
    require(rows(stdout,'FRONTIER_FRAME ')==oracle.frames(capacity),'wrong count/phase sequence')
    require(rows(stdout,'FRONTIER_STORAGE ')==[dict(requested_bytes=requested_bytes(width,height,capacity),arenas=1)],'wrong fresh storage budget')


def check_files(directory,reference,width,height,capacity,strategy):
    for frame in oracle.frames(capacity):
        active=min(frame['active'],capacity)
        mode=9 if active==0 else 0 if capacity>1 and active==capacity else 4
        expected=oracle.identity_image((reference/f'mode-{mode}-frame-{frame["phase"]}.images').read_bytes(),width,height,capacity,strategy)
        require((directory/f'frame-{frame["frame"]}.images').read_bytes()==expected,'range color/depth/identity/guard mismatch')
        require((directory/f'frame-{frame["frame"]}.geometry').read_bytes()==oracle.mesh_expected(frame,strategy),'range geometry/count/guard mismatch')


def check_reuse(stdout,stderr,device,width,height,capacity,strategy,slots,replay,scopes):
    profile(stdout,stderr,device,capacity)
    require(not scopes or strategy=='single','per-draw scopes require separate calls')
    require('Public scene reuse drained and checked' in stdout,'missing reuse drain')
    require(rows(stdout,'RANGE_SCOPE_POLICY ')==[dict(per_draw=scopes)],'wrong scope policy')
    # Sixteen frames cover all eight count variants in BOTH slots.
    frames=[]
    for f in range(16):
        generation=f//slots;state=oracle.frames(capacity)[generation%8]
        frames.append(dict(frame=f,slot=f%slots,generation=generation,phase=state['phase'],active=state['active']))
    require(rows(stdout,'RANGE_FRAME ')==frames,'wrong range slot/generation/count sequence')
    summary=dict(frames=16,slots=slots,capacity=capacity,strategy=oracle.STRATEGIES.index(strategy),replay=int(replay),
        encodes=slots if replay else 16,peak_unretired=slots,requested_bytes=slots*requested_bytes(width,height,capacity))
    require(rows(stdout,'RANGE_SUMMARY ')==[summary],'wrong range ownership/recording summary')
    return summary


def execute(command,directory):
    directory.mkdir()
    with (directory/'stdout').open('w') as stdout,(directory/'stderr').open('w') as stderr:
        result=subprocess.run(command,stdout=stdout,stderr=stderr,timeout=300)
    require(result.returncode==0,'execution failed: '+str(directory))
    return (directory/'stdout').read_text(),(directory/'stderr').read_text()


def files(directory):return {p.name:digest(p) for p in sorted(directory.iterdir()) if p.is_file()}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scale',action='store_true',help='also 1280x720; default 257x193')
    args=parser.parse_args()
    out=Path(tempfile.mkdtemp(prefix='run-',dir=HERE));print(f'Foundation range evidence: {out}',flush=True)
    report=dict(schema=1,complete=False,api='foundation',scope='generated range/count/identity correctness; not timing or native trace evidence',
                runs=[],baseline=[],reuse_runs=[])
    def save():(out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    save()
    try:
        pkg=os.getenv('PKG_CONFIG','pkg-config')
        def variable(name):return subprocess.check_output([pkg,'--variable='+name,'ogpu-foundation'],text=True).strip()
        report['foundation']=int(variable('ogpu_foundation'));require(report['foundation']==1,'unsupported foundation revision')
        report['sdk']=dict(revision=variable('ogpu_revision'),runtime_sha256=digest(Path(variable('libdir'))/'libogpu.so'))
        report['sources']={p.name:digest(p) for p in sorted(HERE.iterdir()) if p.is_file() and p.suffix in ('.c','.h','.py','.slang')}
        report['executables']={name:digest(HERE/name) for name in ('baseline','consumer','reuse','scopes')}
        report['environment']={k:os.getenv(k) for k in ('VK_DRIVER_FILES','VK_ICD_FILENAMES','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC','OGPU_VULKAN_LIBRARY')}
        for width,height in [(257,193)]+([(1280,720)] if args.scale else []):
            baseline=out/f'baseline-{width}x{height}'
            command=[str(HERE/'baseline'),str(width),str(height),'-',str(baseline),'32']
            stdout,stderr=execute(command,baseline)
            device,frames=scene.check_logs(stdout,stderr,32,1,foundation=True)
            if report['baseline']:require(device==report['baseline'][0]['device'],'device changed')
            checks=scene.check_files(baseline,width,height,32,frames)
            report['baseline'].append(dict(directory=baseline.name,extent=[width,height],device=device,command=command,checks=checks,files=files(baseline)))
            save();print(f'{width}x{height}: analytic baseline PASS',flush=True)
            for capacity in (1,64,512):
                for strategy in oracle.STRATEGIES:
                    dest=out/f'{width}x{height}-{capacity}-{strategy}'
                    command=[str(HERE/'consumer'),str(width),str(height),'-',str(dest),str(capacity),strategy]
                    stdout,stderr=execute(command,dest)
                    check_fresh(stdout,stderr,device,width,height,capacity)
                    check_files(dest,baseline,width,height,capacity,strategy)
                    report['runs'].append(dict(directory=dest.name,extent=[width,height],capacity=capacity,strategy=strategy,device=device,command=command,files=files(dest)))
                    save()
                    for scopes in ((False,True) if strategy=='single' else (False,)):
                        for slots in (1,2):
                            for replay in (False,True):
                                policy='replay' if replay else 'reset';executable='scopes' if scopes else 'reuse'
                                case=out/f'{dest.name}-{executable}-{slots}-{policy}'
                                command=[str(HERE/executable),str(width),str(height),'-',str(dest),'16',str(slots),policy,str(capacity),strategy]
                                stdout,stderr=execute(command,case)
                                summary=check_reuse(stdout,stderr,device,width,height,capacity,strategy,slots,replay,scopes)
                                report['reuse_runs'].append(dict(directory=case.name,command=command,per_draw_scopes=scopes,summary=summary,files=files(case)))
                                save()
                    print(f'{dest.name}: fresh identity/count + one/two-slot reset/replay PASS',flush=True)
        report['complete']=True;save();print('Independent foundation ranges PASS',flush=True)
    except Exception as error:report['error']=str(error);save();raise


if __name__=='__main__':main()
