#!/usr/bin/env python3
"""Independent textured scene acceptance; C11, Python and the installed SDK only."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
from textured_checks import check_output,require

HERE=Path(__file__).resolve().parent
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scale',action='store_true',help='also check 1280x720')
    args=parser.parse_args()
    folder=Path(tempfile.mkdtemp(prefix='run-',dir=HERE))
    print(f'Independent textured scene evidence: {folder}',flush=True)
    report=dict(schema=1,complete=False,runs=[],
        scope='installed foundation texture/mip/view/blend correctness; not native comparison, timing or presentation')
    def save():(folder/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    save()
    try:
        pkg=os.getenv('PKG_CONFIG','pkg-config')
        def variable(name):return subprocess.check_output([pkg,'--variable='+name,'ogpu-foundation'],text=True).strip()
        require(variable('ogpu_foundation')=='1','foundation revision mismatch')
        report['sdk']=dict(revision=variable('ogpu_revision'),runtime_sha256=digest(Path(variable('libdir'))/'libogpu.so'),
            header_sha256=digest(Path(variable('includedir'))/'ogpu_next.h'))
        report['sources']={p.name:digest(p) for p in HERE.iterdir() if p.is_file() and p.suffix in ('.c','.h','.py','.slang')}
        report['executable_sha256']=digest(HERE/'consumer')
        report['environment']={k:os.getenv(k) for k in ('VK_DRIVER_FILES','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC','OGPU_VULKAN_LIBRARY')}
        save()
        for width,height in [(257,193),(640,360)]+([(1280,720)] if args.scale else []):
            output=folder/f'{width}x{height}';output.mkdir()
            p=subprocess.run([str(HERE/'consumer'),str(width),str(height),str(output)],text=True,capture_output=True,timeout=300)
            (output/'stdout').write_text(p.stdout);(output/'stderr').write_text(p.stderr)
            require(p.returncode==0,'textured consumer execution failed')
            require(not any(e in p.stdout+p.stderr for e in ('Validation Error:','SYNC-HAZARD','ERROR: AddressSanitizer','runtime error:')),'validation/sanitizer error')
            require('Foundation textured frames drained; independent oracle required' in p.stdout,'missing completion marker')
            devices=[json.loads(line.removeprefix('DEVICE ')) for line in p.stdout.splitlines() if line.startswith('DEVICE ')]
            require(len(devices)==1,'missing/duplicate device identity')
            if report['runs']:require(report['runs'][0]['device']==devices[0],'device changed across extents')
            frames=[json.loads(line.removeprefix('TEXTURED_FRAME ')) for line in p.stdout.splitlines() if line.startswith('TEXTURED_FRAME ')]
            require(frames==[dict(mode=m,frame=f,phase=int(f==1)) for m in range(6) for f in range(3)],'incomplete frame receipt')
            checks=check_output(output,width,height)
            report['runs'].append(dict(extent=[width,height],device=devices[0],directory=output.name,checks=checks,
                files={p.name:digest(p) for p in output.iterdir()}));save()
            print(f'{width}x{height}: 18 independent textured frames PASS',flush=True)
        report['complete']=True;save();print('Independent foundation textured scene PASS')
    except Exception as error:
        report['error']=str(error);save();raise


if __name__=='__main__':main()
