#!/usr/bin/env python3
"""Build/run the foundation M4 textured consumer; native comparison remains separate."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
sys.dont_write_bytecode=True
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
sys.path.insert(0,str(HERE.parent/'compiler'))
import generate
import link_graphics
import oracle
import textured_oracle as texture
spec=importlib.util.spec_from_file_location('scene_control',HERE/'run.py')
scene_control=importlib.util.module_from_spec(spec);spec.loader.exec_module(scene_control)


def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def require(value,message):
    if not value:raise ValueError(message)


def check_output(folder,width,height):
    rows=[]
    for mode in range(len(texture.CASES)):
        for frame in range(3):
            prefix=folder/f'mode-{mode}-frame-{frame}'
            images=prefix.with_suffix('.images').read_bytes()
            require(prefix.with_suffix('.geometry').read_bytes()==oracle.geometry(frame==1,False),
                    'textured geometry/indirect/guard mismatch')
            expected=texture.GUARD+texture.texture(frame==1)+bytes([0xa5])*108
            require(prefix.with_suffix('.texture').read_bytes()==expected,'texture upload/guard mismatch')
            result=texture.check(images,width,height,int(frame==1),mode)
            if frame==2:
                require(images==(folder/f'mode-{mode}-frame-0.images').read_bytes(),'A/B/A image repeat mismatch')
            if mode==2:
                require(images==(folder/f'mode-1-frame-{frame}.images').read_bytes(),'view-relative mip mismatch')
            rows.append(dict(mode=mode,frame=frame,phase=int(frame==1),**result))
    return rows


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check',action='store_true')
    parser.add_argument('--scale',action='store_true')
    args=parser.parse_args()
    parent=ROOT/'target/graphics-scene';parent.mkdir(parents=True,exist_ok=True)
    output=Path(tempfile.mkdtemp(prefix='textured-',dir=parent));print(f'Textured scene evidence: {output}',flush=True)
    report=dict(schema=1,complete=False,api='foundation',runs=[],
        scope='integrated texture/mip/view/blend correctness; not native parity, full M4 or timing acceptance',
        revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        dirty=bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True)),
        environment={k:os.getenv(k) for k in ('VK_DRIVER_FILES','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC','OGPU_VULKAN_LIBRARY')})
    def save():(output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    save()
    try:
        require(not os.getenv('CARGO_TARGET_DIR'),'leave CARGO_TARGET_DIR unset')
        # Generated on every build; no handwritten host layout or stale embedded artifact.
        header=output/'textured_scene.generated.h'
        header.write_text(link_graphics.header(HERE/'textured.vert.slang',HERE/'textured.frag.slang',
            output/'shader',os.getenv('SLANGC','slangc'),'textured_scene',native_heaps=True,api='foundation'))
        for name in ('scene_prepare','scene_prepare16'):
            source=HERE/('prepare16.slang' if name.endswith('16') else 'prepare.slang')
            reflection,assembly,binary=generate.compile_source(source,output/name,os.getenv('SLANGC','slangc'))
            require((HERE/f'foundation_{name}.generated.h').read_text()==generate.header(
                reflection,assembly,binary,source.read_bytes(),name,api='foundation'),'stale compute artifact: '+name)
        subprocess.run([sys.executable,'-B',str(HERE/'test_textured_oracle.py')],check=True)
        subprocess.run([os.getenv('CARGO','cargo'),'build','--locked','--release','-p','ogpu','--no-default-features'],cwd=ROOT,check=True)
        binary=output/'consumer';library=ROOT/'target/release'
        command=[*shlex.split(os.getenv('CC','cc')),'-std=c11','-O2','-Wall','-Wextra','-Werror',
            '-I'+str(ROOT/'include'),'-I'+str(output),'-I'+str(HERE),str(HERE/'textured.c'),
            '-L'+str(library),'-Wl,-rpath,'+str(library),'-logpu','-o',str(binary)]
        subprocess.run(command,check=True)
        failure=output/'failures'
        failure_command=[str(failure) if p==str(binary) else str(HERE/'foundation_failures.c') if p==str(HERE/'textured.c') else p for p in command]
        failure_command+=['-DSCENE_TEXTURE_FAILURES']+['-Wl,--wrap=ogpu_next_'+name for name in
            ('commands_end','queue_submit','timeline_wait','arena_destroy','memory_destroy','device_destroy')]
        subprocess.run(failure_command,check=True)
        symbols=subprocess.check_output(['nm','-u',str(binary)],text=True)
        imported={line.split()[-1] for line in symbols.splitlines() if line.split()}
        require('ogpu_next_draw_indexed_indirect' in imported and
                all(not name.startswith('vk') and (not name.startswith('ogpu_') or name.startswith('ogpu_next_')) for name in imported),
                'textured public consumer boundary mismatch')
        report.update(compile_command=command,executable_sha256=digest(binary),runtime_sha256=digest(library/'libogpu.so'),
            generated_sha256=digest(header),compiler=generate.VERSION,
            failure_control=dict(compile_command=failure_command,executable_sha256=digest(failure),runs=[]))
        sources=[HERE/n for n in ('textured.c','foundation.c','textured.py','textured_oracle.py','oracle.py',
            'textured.vert.slang','textured.frag.slang','foundation_failures.c','run.py','prepare.slang','prepare16.slang',
            'foundation_scene_prepare.generated.h','foundation_scene_prepare16.generated.h')]
        sources+=list((ROOT/'examples/compiler').glob('*.py'))
        sources += list((ROOT/'crates/ogpu/src').rglob('*.rs'))+[ROOT/'include/ogpu_next.h']
        report['sources']={str(p.relative_to(ROOT)):digest(p) for p in sources}
        if args.check:
            report['build_only']=True;report['complete']=True;save();print('Textured scene build/oracle PASS; no GPU');return
        require(os.getenv('VK_DRIVER_FILES') and 'VK_LAYER_KHRONOS_validation' in os.getenv('VK_INSTANCE_LAYERS','')
                and os.getenv('VK_LAYER_VALIDATE_SYNC')=='1' and not os.getenv('VK_LOADER_LAYERS_DISABLE'),
                'select one ICD and enable Vulkan/synchronization validation')
        env=dict(os.environ,LD_LIBRARY_PATH=str(library)+':'+os.getenv('LD_LIBRARY_PATH',''))
        for mode in ('end','submit','unknown','wait'):
            folder=output/('failure-'+mode);folder.mkdir()
            p=subprocess.run([str(failure),mode,'257','193',str(folder)],capture_output=True,text=True,env=env,timeout=60)
            (folder/'stdout').write_text(p.stdout);(folder/'stderr').write_text(p.stderr)
            scene_control.check_failure(mode,p.returncode,p.stdout,p.stderr,allocations=12,frame_prefix='TEXTURED_FRAME ')
            report['failure_control']['runs'].append(dict(mode=mode,returncode=p.returncode,
                files={f.name:digest(f) for f in folder.iterdir()}));save()
        for width,height in [(257,193),(640,360)]+([(1280,720)] if args.scale else []):
            folder=output/f'{width}x{height}';folder.mkdir()
            p=subprocess.run([str(binary),str(width),str(height),str(folder)],capture_output=True,text=True,env=env,timeout=300)
            (folder/'stdout').write_text(p.stdout);(folder/'stderr').write_text(p.stderr)
            require(p.returncode==0 and not any(e in p.stdout+p.stderr for e in ('Validation Error:','SYNC-HAZARD')),'textured execution/validation failure')
            require('Foundation textured frames drained; independent oracle required' in p.stdout,'missing completion marker')
            frames=[json.loads(line.removeprefix('TEXTURED_FRAME ')) for line in p.stdout.splitlines() if line.startswith('TEXTURED_FRAME ')]
            require(frames==[dict(mode=m,frame=f,phase=int(f==1)) for m in range(6) for f in range(3)],'incomplete textured frames')
            devices=[json.loads(line.removeprefix('DEVICE ')) for line in p.stdout.splitlines() if line.startswith('DEVICE ')]
            require(len(devices)==1,'missing device identity')
            if report['runs']:require(devices[0]==report['runs'][0]['device'],'device changed across extents')
            checks=check_output(folder,width,height)
            report['runs'].append(dict(extent=[width,height],device=devices[0],checks=checks,directory=folder.name,
                files={p.name:digest(p) for p in folder.iterdir()}));save()
            print(f'{width}x{height}: 18 textured/mip/view/blend/geometry frames PASS',flush=True)
        report['complete']=True;save();print('Foundation textured scene PASS; native comparison still pending')
    except Exception as error:
        report['error']=str(error);save();raise


if __name__=='__main__':main()
