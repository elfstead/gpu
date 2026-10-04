#!/usr/bin/env python3
"""Check the copied generated indexed/depth scene without a checkout or compiler."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
sys.dont_write_bytecode=True
import oracle
HERE=Path(__file__).resolve().parent


def require(value,message):
    if not value:raise ValueError(message)


def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()


def rows(text,prefix):
    return [json.loads(line[len(prefix):]) for line in text.splitlines() if line.startswith(prefix)]


def check_logs(stdout,stderr,bits,abi):
    require('Validation Error:' not in stdout+stderr and 'SYNC-HAZARD' not in stdout+stderr,'validation failure')
    require('Public indexed/depth frames drained; CPU oracle must independently accept outputs' in stdout,'missing drain gate')
    require(rows(stdout,'PUBLIC_SCENE ')==[dict(abi=abi,index_bytes=bits//8,gpu_generated=True)],'wrong public configuration')
    require(rows(stdout,'GENERATED_SCENE ')==[dict(compute_push_bytes=32,vertex_push_bytes=8)],'wrong generated scene profile')
    devices=rows(stdout,'DEVICE ');require(len(devices)==1,'missing/duplicate device')
    frames=rows(stdout,'SCENE_FRAME ')
    require(frames==[dict(mode=m,phase=int(f==1),frame=f) for m in range(10) for f in range(3)],'missing/reordered frame records')
    return devices[0],frames


def check_files(directory,width,height,bits,frames):
    checks=[]
    for frame in frames:
        stem=directory/f'mode-{frame["mode"]}-frame-{frame["frame"]}'
        checks.append(dict(**frame,**oracle.check(stem.with_suffix('.images').read_bytes(),stem.with_suffix('.geometry').read_bytes(),
                                                width,height,frame['phase'],frame['mode'],bits//8)))
    for mode in range(10):
        for suffix in ('images','geometry'):
            require((directory/f'mode-{mode}-frame-0.{suffix}').read_bytes()==(directory/f'mode-{mode}-frame-2.{suffix}').read_bytes(),'A/B/A mismatch')
    for frame in range(3):
        normal=(directory/f'mode-0-frame-{frame}.images').read_bytes()
        for mode in (1,6):require(normal==(directory/f'mode-{mode}-frame-{frame}.images').read_bytes(),'draw-order/LOAD mismatch')
    require((directory/'mode-0-frame-0.images').read_bytes()!=(directory/'mode-0-frame-1.images').read_bytes(),'geometry did not move')
    return checks


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scale',action='store_true',help='also 1280x720; default 257x193')
    args=parser.parse_args()
    out=Path(tempfile.mkdtemp(prefix='run-',dir=HERE));print(f'Indexed scene evidence: {out}',flush=True)
    report=dict(schema=1,complete=False,scope='generated installed indexed/depth correctness; not timing or full M4',runs=[])
    def save():(out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    save()
    try:
        pkg=os.getenv('PKG_CONFIG','pkg-config')
        report['revision']=subprocess.check_output([pkg,'--variable=ogpu_revision','ogpu'],text=True).strip()
        report['abi']=int(subprocess.check_output([pkg,'--variable=ogpu_abi','ogpu'],text=True))
        report['sources']={p.name:digest(p) for p in sorted(HERE.iterdir()) if p.is_file() and p.suffix in ('.c','.h','.py','.slang')}
        report['executable_sha256']=digest(HERE/'scene')
        report['environment']={k:os.getenv(k) for k in ('VK_DRIVER_FILES','VK_ICD_FILENAMES','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC','OGPU_VULKAN_LIBRARY')}
        for width,height in [(257,193)]+([(1280,720)] if args.scale else []):
            wide=None
            for bits in (32,16):
                dest=out/f'{width}x{height}-{bits}';dest.mkdir()
                command=[str(HERE/'scene'),str(width),str(height),'-',str(dest),str(bits)]
                with (dest/'stdout').open('w') as stdout,(dest/'stderr').open('w') as stderr:
                    result=subprocess.run(command,stdout=stdout,stderr=stderr,timeout=300)
                require(result.returncode==0,'scene execution failed: '+str(dest))
                device,frames=check_logs((dest/'stdout').read_text(),(dest/'stderr').read_text(),bits,report['abi'])
                if report['runs']:require(device==report['runs'][0]['device'],'device changed')
                checks=check_files(dest,width,height,bits,frames)
                if wide:
                    for frame in frames:
                        name=f'mode-{frame["mode"]}-frame-{frame["frame"]}.images'
                        require((dest/name).read_bytes()==(wide/name).read_bytes(),'index-width image mismatch')
                else:wide=dest
                report['runs'].append(dict(directory=dest.name,extent=[width,height],index_bytes=bits//8,device=device,command=command,
                    checks=checks,files={p.name:digest(p) for p in sorted(dest.iterdir()) if p.is_file()}))
                save();print(f'{width}x{height} UINT{bits}: 30 analytic/geometry/guard/repeat frames PASS',flush=True)
        report['complete']=True;save();print('Installed generated indexed/depth scene PASS',flush=True)
    except Exception as error:report['error']=str(error);save();raise


if __name__=='__main__':main()
