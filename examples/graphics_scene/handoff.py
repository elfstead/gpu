#!/usr/bin/env python3
"""Compare relocated indexed-scene outputs with direct Vulkan using exact SDK artifacts."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import struct
import subprocess
import sys
import tempfile
sys.dont_write_bytecode=True
import oracle
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]


def require(value,message):
    if not value:raise ValueError(message)


def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()


def binary(header,name):
    matches=re.findall(r'static const uint32_t '+re.escape(name)+r'_code\[\] = \{(.*?)\};',header,re.S)
    require(len(matches)==1,'missing/duplicate embedded native artifact')
    words=[int(v,16) for v in re.findall(r'0x([0-9a-f]+)u',matches[0])]
    require(len(words)>=5 and words[0]==0x07230203,'invalid embedded SPIR-V')
    return struct.pack('<'+'I'*len(words),*words)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prefix',required=True,type=Path)
    parser.add_argument('--consumer',required=True,type=Path,help='copied indexed scene directory with successful run-* reports')
    parser.add_argument('--development',action='store_true',help='allow dirty provenance; never acceptance')
    args=parser.parse_args()
    out=Path(tempfile.mkdtemp(prefix='handoff-native-',dir=ROOT/'target/graphics-scene'));print(f'Scene handoff native evidence: {out}',flush=True)
    report=dict(schema=1,complete=False,development=args.development,runs=[],
        revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        dirty=bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True)))
    def save():(out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    save()
    try:
        require(os.getenv('VK_DRIVER_FILES') and 'VK_LAYER_KHRONOS_validation' in os.getenv('VK_INSTANCE_LAYERS','')
                and os.getenv('VK_LAYER_VALIDATE_SYNC')=='1' and not os.getenv('VK_LOADER_LAYERS_DISABLE'),'select ICD and validation/sync')
        manifest_path=args.prefix/'share/ogpu/manifest.json';manifest=json.loads(manifest_path.read_text())
        require(args.development or (not manifest['source_dirty'] and not report['dirty'] and manifest['revision']==report['revision']),
                'clean matching SDK/source required')
        for relative,h in manifest['files_sha256'].items():require(digest(args.prefix/relative)==h,'changed SDK file: '+relative)
        report['sdk']=dict(path=str(manifest_path.resolve()),sha256=digest(manifest_path),revision=manifest['revision'],dirty=manifest['source_dirty'])
        artifacts=args.prefix/'share/ogpu/examples/indexed-scene'
        consumers=[]
        for path in sorted(args.consumer.glob('run-*/report.json')):
            c=json.loads(path.read_text())
            require(c.get('complete') and c['revision']==manifest['revision'] and c['abi']==manifest['abi'],'incomplete/wrong consumer provenance')
            require(len(c['runs'])==2 and {(tuple(r['extent']),r['index_bytes']) for r in c['runs']}=={((257,193),2),((257,193),4)},'expected complete small index-width handoff')
            consumers.append((path,c))
        require(consumers,'no successful consumer reports')
        report['consumers']=[dict(path=str(p.resolve()),sha256=digest(p)) for p,_ in consumers]
        cc=shlex.split(os.getenv('CC','cc'))
        flags=[*cc,'-std=c11','-O2','-Wall','-Wextra','-Werror','-I'+str(ROOT/'include'),
            '-I'+str(ROOT/'vendor/Vulkan-Headers/include'),'-I'+str(ROOT/'examples/learned_image/generated')]
        command=[*flags,str(HERE/'native.c'),'-ldl','-o',str(out/'native')]
        subprocess.run(command,check=True)
        require('ogpu' not in subprocess.check_output(['nm','-u',str(out/'native')],text=True).lower(),'native control links OGPU')
        deps=subprocess.check_output([*flags,'-MM',str(HERE/'native.c')],text=True)
        report['sources']={str(Path(p).resolve()):digest(Path(p)) for p in shlex.split(deps.replace('\\\n','').split(':',1)[1])}
        report['sources'][str(Path(__file__).resolve())]=digest(Path(__file__))
        report['sources'][str(HERE/'oracle.py')]=digest(HERE/'oracle.py')
        report['build']=dict(command=command,sha256=digest(out/'native'),compiler=subprocess.check_output([*cc,'--version'],text=True).splitlines()[0])
        report['environment']={k:os.getenv(k) for k in ('VK_DRIVER_FILES','VK_ICD_FILENAMES','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC','OGPU_VULKAN_LIBRARY')}
        expected=[dict(mode=m,phase=int(f==1),frame=f) for m in range(10) for f in range(3)]
        for bits in (32,16):
            dest=out/str(bits);dest.mkdir();shaders={}
            for stage,header,name in [('compute','scene_prepare'+('16' if bits==16 else ''),'scene_prepare'+('16' if bits==16 else '')),
                                      ('vertex','scene_pair','scene_pair_vertex'),('fragment','scene_pair','scene_pair_fragment')]:
                data=binary((artifacts/(header+'.generated.h')).read_text(),name)
                (dest/(stage+'.spv')).write_bytes(data);shaders[stage]=digest(dest/(stage+'.spv'))
            command=[str(out/'native'),'257','193',str(dest),str(dest),str(bits)]
            with (dest/'stdout').open('w') as stdout,(dest/'stderr').open('w') as stderr:
                executed=subprocess.run(command,stdout=stdout,stderr=stderr,timeout=300)
            stdout=(dest/'stdout').read_text();stderr=(dest/'stderr').read_text()
            require(executed.returncode==0 and 'Validation Error:' not in stdout+stderr and 'SYNC-HAZARD' not in stdout+stderr,'native execution/validation failed')
            require('Native indexed/depth frames drained' in stdout,'missing native drain gate')
            def rows(prefix):return [json.loads(line[len(prefix):]) for line in stdout.splitlines() if line.startswith(prefix)]
            require(rows('SCENE_FRAME ')==expected and len(rows('DEVICE '))==1,'native frame/device mismatch')
            device=rows('DEVICE ')[0];checks=[]
            for frame in expected:
                stem=dest/f'mode-{frame["mode"]}-frame-{frame["frame"]}'
                checks.append(dict(**frame,**oracle.check(stem.with_suffix('.images').read_bytes(),stem.with_suffix('.geometry').read_bytes(),257,193,frame['phase'],frame['mode'],bits//8)))
            compared=0
            for path,c in consumers:
                matches=[r for r in c['runs'] if r['index_bytes']==bits//8]
                require(len(matches)==1 and matches[0]['device']==device and len(matches[0]['checks'])==30,'consumer device/frame mismatch')
                run=matches[0];folder=path.parent/run['directory']
                for name,h in run['files'].items():require(digest(folder/name)==h,'changed consumer output/log')
                for frame in expected:
                    for suffix in ('images','geometry'):
                        name=f'mode-{frame["mode"]}-frame-{frame["frame"]}.{suffix}'
                        data=(dest/name).read_bytes();require(data==(folder/name).read_bytes(),'native/installed byte mismatch: '+name)
                        compared+=len(data)
            report['runs'].append(dict(index_bytes=bits//8,device=device,command=command,shaders=shaders,checks=checks,compared_bytes=compared,
                files={p.name:digest(p) for p in sorted(dest.iterdir()) if p.is_file()}))
            save();print(f'UINT{bits}: exact SDK native artifacts, analytic checks and all installed outputs PASS',flush=True)
        report['complete']=True;save();print('Scene native/installed handoff '+('DEVELOPMENT' if args.development else 'PASS'))
    except Exception as error:report['error']=str(error);save();raise


if __name__=='__main__':main()
