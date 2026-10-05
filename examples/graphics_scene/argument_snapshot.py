#!/usr/bin/env python3
"""Observable argument payload and same-address snapshot controls. Not timing."""
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
import frontier as f
import oracle
ROOT,HERE=f.ROOT,f.HERE
require,digest=f.require,f.digest
WIDTH,HEIGHT=257,193
IMAGE_BYTES=WIDTH*HEIGHT*8+256
SIZES=(8,64,256)


def color(size,surface,changed):
    require(size in SIZES and surface in (0,1) and changed in (0,1),'invalid payload case')
    checksum=sum((i+1)*(i+1+changed) for i in range((size-8)//4))
    identity=2+surface+4*checksum
    return bytes((identity&255,(identity>>8)&255,255,255))


def check(data,size):
    require(len(data)==4*IMAGE_BYTES,'wrong snapshot file size')
    images=[data[i*IMAGE_BYTES:(i+1)*IMAGE_BYTES] for i in range(4)]
    require(images[0]==images[1]==images[3] and images[2]!=images[0],'missing equal/equal/changed/equal snapshots')
    pixels=WIDTH*HEIGHT;checks=[]
    for snapshot,image in enumerate(images):
        phase=int(snapshot==2);checked=0
        colors={oracle.BLACK:oracle.BLACK,oracle.RED:color(size,0,0),oracle.GREEN:color(size,1,phase)}
        require(image[:64]==oracle.GUARD and image[64+pixels*4:192+pixels*4]==oracle.GUARD*2
                and image[-64:]==oracle.GUARD,'snapshot guards changed')
        depths=struct.unpack('<'+str(pixels)+'f',image[192+pixels*4:-64])
        for row in range(HEIGHT):
            y=2*(row+.5)/HEIGHT-1
            for column in range(WIDTH):
                x=2*(column+.5)/WIDTH-1
                if not oracle.interior(x,y,WIDTH,HEIGHT,phase):continue
                baseline,depth=oracle.expected(x,y,phase,0)
                wanted=colors[baseline]
                index=row*WIDTH+column
                require(image[64+index*4:68+index*4]==wanted and depths[index]==depth,
                        f'snapshot {snapshot} payload/depth mismatch at {column},{row}')
                checked+=1
        require(checked>pixels*.9,'insufficient analytic coverage')
        checks.append(dict(snapshot=snapshot,checked_pixels=checked,changed=bool(phase)))
    return checks


def check_log(stdout,stderr,backend,size):
    require('Validation Error:' not in stdout+stderr and 'SYNC-HAZARD' not in stdout+stderr,'validation failure')
    require('Argument snapshot control drained' in stdout,'missing drain gate')
    require(f.rows(stdout,'ARGUMENT_SNAPSHOTS ')==[dict(bytes=size,snapshots=4,draws=8,
            invalid_retries=0 if backend=='native' else 8)],'wrong snapshot/retry profile')
    limits=f.rows(stdout,'ARGUMENT_LIMIT ');device=f.rows(stdout,'DEVICE ')
    require(len(limits)==len(device)==1 and limits[0]['max_push_data_bytes']>=size,'missing device/limit')
    if backend=='public':require(f.rows(stdout,'PUBLIC_SCENE ')==[dict(abi=19,index_bytes=4,gpu_generated=True)],'wrong public profile')
    else:require(f.rows(stdout,'DRAW_IDENTITY_FEATURES ')==[dict(shaderDrawParameters=True)],'missing native draw identity')
    return limits[0],device[0]


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--build-only',action='store_true')
    args=parser.parse_args()
    base=ROOT/'target/graphics-scene';base.mkdir(parents=True,exist_ok=True)
    out=Path(tempfile.mkdtemp(prefix='argument-snapshot-',dir=base));print(f'Argument snapshot evidence: {out}',flush=True)
    report=dict(schema=1,complete=False,build_only=args.build_only,scope=__doc__,runs=[],unsupported=[],builds={},shaders={},
        revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        dirty=bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True)))
    def save():(out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    save()
    try:
        require(not os.getenv('CARGO_TARGET_DIR'),'leave CARGO_TARGET_DIR unset')
        if not args.build_only:
            require(os.getenv('VK_DRIVER_FILES') and 'VK_LAYER_KHRONOS_validation' in os.getenv('VK_INSTANCE_LAYERS','')
                    and os.getenv('VK_LAYER_VALIDATE_SYNC')=='1' and not os.getenv('VK_LOADER_LAYERS_DISABLE'),
                    'select ICD and enable validation/sync')
        report['environment']={k:os.getenv(k) for k in ('VK_DRIVER_FILES','VK_ICD_FILENAMES','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC','OGPU_VULKAN_LIBRARY')}
        subprocess.run(['cargo','build','--locked','--release','-p','ogpu'],cwd=ROOT,check=True)
        report['runtime_sha256']=digest(ROOT/'target/release/libogpu.so')
        cc=shlex.split(os.getenv('CC','cc'));report['compiler']=subprocess.check_output([*cc,'--version'],text=True).splitlines()[0]
        report['slang']=f.scene.generate.VERSION
        limits={};devices={};sources={}
        for size in SIZES:
            if limits and any(limit['max_push_data_bytes']<size for limit in limits.values()):
                report['unsupported'].append(dict(bytes=size,limits=limits.copy()));save();continue
            folder=out/str(size);folder.mkdir()
            wrapper=folder/'vertex.slang'
            wrapper.write_text(f'#define ARGUMENT_BYTES {size}\n#include '+json.dumps(str(HERE/'argument.vert.slang'))+'\n')
            report['shaders'][str(size)]={}
            for stage,source in [('compute',HERE/'prepare.slang'),('vertex',wrapper),('fragment',HERE/'identity.frag.slang')]:
                reflected,assembly,binary=f.scene.generate.compile_source(source,folder/stage,os.getenv('SLANGC','slangc'),stage)
                if stage=='vertex':
                    root=reflected['parameters'][0]['type']['elementType']
                    require(f.scene.generate.uniform_size(root)==(size,8),'wrong reflected root extent')
                    fields=[(x['name'],x['binding']['offset'],x['binding']['size']) for x in root['fields']]
                    require(fields==[('vertices',0,8)]+([('payload',8,size-8)] if size>8 else []),'wrong payload layout')
                    require('BuiltIn DrawIndex' in assembly,'lost local draw identity')
                path=folder/(stage+'.spv');path.write_bytes(binary)
                report['shaders'][str(size)][stage]=dict(sha256=digest(path),reflection=reflected)
            results=[]
            for backend in ('native','public'):
                target=folder/backend;target.mkdir()
                flags=[*cc,'-std=c11','-O2','-Wall','-Wextra','-Werror',f'-DSCENE_RASTER_BYTES={size}',
                    '-I'+str(ROOT/'include')]
                if backend=='native':flags+=['-DARGUMENT_NATIVE','-I'+str(ROOT/'vendor/Vulkan-Headers/include'),'-I'+str(ROOT/'examples/learned_image/generated')]
                command=[*flags,str(HERE/'argument_snapshot.c'),'-ldl']
                if backend=='public':command+=['-L'+str(ROOT/'target/release'),'-Wl,-rpath,'+str(ROOT/'target/release'),'-logpu']
                command+=['-o',str(target/'probe')];subprocess.run(command,check=True)
                deps=subprocess.check_output([*flags,'-MM',str(HERE/'argument_snapshot.c')],text=True)
                for path in shlex.split(deps.replace('\\\n','').split(':',1)[1]):sources[str(Path(path).resolve())]=digest(Path(path))
                symbols=subprocess.check_output(['nm','-u',str(target/'probe')],text=True)
                require(('ogpu' not in symbols.lower()) if backend=='native' else ('ogpu_batch_draw_indexed_indirect' in symbols
                        and not any(line.split()[-1].startswith('vk') for line in symbols.splitlines() if line.split())), 'wrong linkage')
                report['builds'][f'{size}-{backend}']=dict(command=command,sha256=digest(target/'probe'))
                if args.build_only:continue
                command=[str(target/'probe'),str(folder),str(target)]
                with (target/'stdout').open('w') as stdout,(target/'stderr').open('w') as stderr:
                    result=subprocess.run(command,stdout=stdout,stderr=stderr,timeout=300)
                require(result.returncode==0,'snapshot execution failed: '+str(target))
                limit,device=check_log((target/'stdout').read_text(),(target/'stderr').read_text(),backend,size)
                if backend in limits:require(limit==limits[backend] and device==devices[backend],'device/limit changed between root sizes')
                limits[backend],devices[backend]=limit,device
                require((target/'final.geometry').read_bytes()==oracle.geometry(0,False)+bytes([0xa5])*4,'geometry/guard mismatch')
                data=(target/'snapshots.images').read_bytes();checks=check(data,size);results.append(data)
                report['runs'].append(dict(bytes=size,backend=backend,command=command,device=devices[backend],limit=limits[backend],checks=checks,
                    files={p.name:digest(p) for p in sorted(target.iterdir()) if p.is_file()}));save()
            if not args.build_only:
                require(devices['native']==devices['public'] and limits['native']==limits['public'],'native/public device/limit mismatch')
                require(len(results)==2 and results[0]==results[1],'native/public full-byte snapshot mismatch')
                print(f'{size}-byte roots: same-address mutation, rejected-call retry, overwrite and native byte match PASS',flush=True)
        for p in [Path(__file__),HERE/'argument.vert.slang',HERE/'prepare.slang',HERE/'identity.frag.slang',HERE/'oracle.py',HERE/'frontier.py']:
            sources[str(p.resolve())]=digest(p)
        report.update(sources=sources,complete=True);save()
        print('Argument snapshot '+('BUILD ONLY' if args.build_only else 'PASS; strategy/timing audit remains open'))
    except Exception as error:report['error']=str(error);save();raise


if __name__=='__main__':main()
