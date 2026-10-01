#!/usr/bin/env python3
"""Validate the native M4 reference, optionally matched against the public ABI-18 scene."""
import argparse
import hashlib
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
import oracle


def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()


def compare_outputs(native, public, rows):
    """Compare every byte including excluded raster edges and all padding/guards."""
    compared=0
    for row in rows:
        for suffix in ('images','geometry'):
            name=f"mode-{row['mode']}-frame-{row['frame']}.{suffix}"
            expected=(native/name).read_bytes();actual=(public/name).read_bytes()
            generate.require(actual==expected,f'public/native byte mismatch: {name}')
            compared+=len(actual)
    return dict(frames=len(rows),files=2*len(rows),compared_bytes=compared,exact=True)


def compare_index_width(reference_root, reference_run, output, rows):
    compared=0
    for row in rows:
        name=f"mode-{row['mode']}-frame-{row['frame']}.images"
        path=reference_root/reference_run['directory']/name
        generate.require(digest(path)==reference_run['files'][name],'UINT32 reference artifact hash mismatch')
        expected=path.read_bytes()
        generate.require(expected==(output/name).read_bytes(),f'index-width image mismatch: {name}')
        compared+=len(expected)
    return dict(frames=len(rows),files=len(rows),compared_bytes=compared,exact=True)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check',action='store_true')
    parser.add_argument('--scale',action='store_true')
    parser.add_argument('--public',action='store_true',help='also build/run public API consumer and require exact native agreement')
    parser.add_argument('--index16',action='store_true',help='generate and bind UINT16 instead of UINT32 indices')
    parser.add_argument('--reference32',type=Path,help='accepted matched UINT32 report; required for UINT16 GPU acceptance')
    args=parser.parse_args()
    build=ROOT/'target/graphics-scene';build.mkdir(parents=True,exist_ok=True)
    dest=Path(tempfile.mkdtemp(prefix='matched-' if args.public else 'native-',dir=build))
    print(f'Scene evidence: {dest}',flush=True)
    report=dict(schema=3,index_bytes=2 if args.index16 else 4,complete=False,
                scope='matched public/native indexed/depth correctness; not full M4 or performance acceptance' if args.public else
                    'native indexed/depth correctness, not public API acceptance or performance',
                revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
                dirty=bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True)),runs=[],shaders={},
                environment={k:os.getenv(k) for k in ('VK_DRIVER_FILES','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC','OGPU_VULKAN_LIBRARY')})
    report['toolchain']=dict(slang=generate.VERSION,cc=subprocess.check_output([*shlex.split(os.getenv('CC','cc')),'--version'],text=True).splitlines()[0],
                             python=sys.version.split()[0])
    def save(): (dest/'report.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
    save()
    try:
        reference=None
        if args.reference32:
            generate.require(args.index16 and args.public,'UINT32 reference is only for matched UINT16 runs')
            reference=json.loads(args.reference32.read_text())
            generate.require(reference.get('complete') and not reference.get('dirty') and not reference.get('build_only')
                             and reference.get('schema') in (2,3) and reference.get('index_bytes',4)==4,
                             'UINT32 reference must be a clean accepted matched GPU report')
            report['reference32']=dict(path=str(args.reference32.resolve()),sha256=digest(args.reference32),revision=reference['revision'])
        generate.require(args.check or not args.index16 or reference is not None,'UINT16 GPU acceptance requires --public --reference32')
        sources=list(HERE.glob('*.py'))+list(HERE.glob('*.c'))+list(HERE.glob('*.slang'))+[HERE/'README.md']
        sources += [ROOT/'examples/learned_image'/name for name in ('native.c','native_workload.h','extent.h')]
        sources += list((ROOT/'examples/learned_image/generated').glob('*.h'))
        sources += list((ROOT/'examples/compiler').glob('*.py'))
        sources += [ROOT/'include/ogpu.h',ROOT/'vendor/Vulkan-Headers/include/vulkan/vulkan_core.h']
        report['sources']={str(p.relative_to(ROOT)):digest(p) for p in sorted(sources)}
        for name,stage in [('prepare16' if args.index16 else 'prepare','compute'),('scene.vert','vertex'),('scene.frag','fragment')]:
            reflection,assembly,binary=generate.compile_source(HERE/(name+'.slang'),dest/stage,os.getenv('SLANGC','slangc'),stage)
            if stage=='compute':
                generate.require(reflection['entryPoints'][0]['threadGroupSize']==[8,1,1],'wrong workgroup')
            if stage!='fragment':
                parameters=reflection['parameters'];generate.require(len(parameters)==1,'wrong native root count')
                root=parameters[0]['type']['elementType']
                fields=[(field['name'],field['binding']['offset'],field['binding']['size']) for field in root['fields']]
                wanted=[('vertices',0,8)]+([('indices',8,8),('draws',16,8),('phase',24,4),('empty',28,4)] if stage=='compute' else [])
                generate.require(fields==wanted and generate.uniform_size(root)==(32 if stage=='compute' else 8,8),'native root layout mismatch')
            # This uses the native compiler, not inspect()/header()'s installed subset.
            (dest/(stage+'.spv')).write_bytes(binary)
            report['shaders'][stage]=dict(sha256=digest(dest/(stage+'.spv')),reflection=reflection)
        binary=dest/'native'
        command=[*shlex.split(os.getenv('CC','cc')),'-std=c11','-O2','-Wall','-Wextra','-Werror',
                 '-I'+str(ROOT/'include'),'-I'+str(ROOT/'vendor/Vulkan-Headers/include'),
                 '-I'+str(ROOT/'examples/learned_image/generated'),str(HERE/'native.c'),'-ldl','-o',str(binary)]
        subprocess.run(command,check=True)
        symbols=subprocess.check_output(['nm','-u',str(binary)],text=True)
        generate.require('ogpu' not in symbols.lower(),'native reference links OGPU')
        report['executable_sha256']=digest(binary); report['compile_command']=command
        executables={'native':binary}
        if args.public:
            generate.require(not os.getenv('CARGO_TARGET_DIR'),'leave CARGO_TARGET_DIR unset')
            subprocess.run([os.getenv('CARGO','cargo'),'build','--locked','--release','-p','ogpu'],cwd=ROOT,check=True)
            public=dest/'public'
            public_command=[*shlex.split(os.getenv('CC','cc')),'-std=c11','-O2','-Wall','-Wextra','-Werror',
                            '-I'+str(ROOT/'include'),str(HERE/'public.c'),'-L'+str(ROOT/'target/release'),
                            '-Wl,-rpath,'+str(ROOT/'target/release'),'-logpu','-o',str(public)]
            subprocess.run(public_command,check=True)
            symbols=subprocess.check_output(['nm','-u',str(public)],text=True)
            generate.require('ogpu_batch_draw_indexed_indirect' in symbols and not any(
                line.split()[-1].startswith('vk') for line in symbols.splitlines() if line.split()),'public consumer boundary mismatch')
            report['public']=dict(executable_sha256=digest(public),compile_command=public_command,
                                  runtime_sha256=digest(ROOT/'target/release/libogpu.so'),abi=19)
            executables['public']=public
        subprocess.run([sys.executable,'-B',str(HERE/'test_oracle.py')],check=True)
        subprocess.run([sys.executable,'-B',str(HERE/'test_runner.py')],check=True)
        if args.check:
            report['build_only']=True;report['complete']=True;save();print('Scene build/oracle gates PASS; no GPU');return
        generate.require(os.getenv('VK_DRIVER_FILES') and 'VK_LAYER_KHRONOS_validation' in os.getenv('VK_INSTANCE_LAYERS','')
                         and os.getenv('VK_LAYER_VALIDATE_SYNC')=='1' and not os.getenv('VK_LOADER_LAYERS_DISABLE'),
                         'select one ICD and enable Vulkan/synchronization validation')
        environment=os.environ.copy()
        if args.public:
            environment['LD_LIBRARY_PATH']=str(ROOT/'target/release')+':'+environment.get('LD_LIBRARY_PATH','')
            report['public']['LD_LIBRARY_PATH']=environment['LD_LIBRARY_PATH']
        for width,height in [(257,193),(640,360)]+([(1280,720)] if args.scale else []):
            for backend,binary in executables.items():
                output=dest/(f'{width}x{height}-{backend}' if args.public else f'{width}x{height}');output.mkdir()
                with (output/'stdout').open('w') as out,(output/'stderr').open('w') as err:
                    run=subprocess.run([str(binary),str(width),str(height),str(dest),str(output),'16' if args.index16 else '32'],stdout=out,stderr=err,timeout=300,env=environment)
                stdout=(output/'stdout').read_text();stderr=(output/'stderr').read_text()
                generate.require(run.returncode==0 and 'Validation Error:' not in stdout+stderr,f'{backend} execution/validation failed; inspect retained logs')
                generate.require(f'{backend.title()} indexed/depth frames drained; CPU oracle must independently accept outputs' in stdout,'missing drain gate')
                if backend=='public':
                    context=[json.loads(line.removeprefix('PUBLIC_SCENE ')) for line in stdout.splitlines() if line.startswith('PUBLIC_SCENE ')]
                    generate.require(context==[dict(abi=19,index_bytes=report['index_bytes'],gpu_generated=True)],'wrong public scene configuration')
                devices=[json.loads(line.removeprefix('DEVICE ')) for line in stdout.splitlines() if line.startswith('DEVICE ')]
                generate.require(len(devices)==1,'missing/duplicate device identity')
                if report['runs']: generate.require(devices[0]==report['runs'][0]['device'],'device changed across extents')
                rows=[json.loads(line.removeprefix('SCENE_FRAME ')) for line in stdout.splitlines() if line.startswith('SCENE_FRAME ')]
                expected=[dict(mode=m,phase=int(f==1),frame=f) for m in range(10) for f in range(3)]
                generate.require(rows==expected,'missing/reordered frame records')
                checks=[]
                for row in rows:
                    stem=output/f"mode-{row['mode']}-frame-{row['frame']}"
                    checks.append(dict(**row,**oracle.check(stem.with_suffix('.images').read_bytes(),stem.with_suffix('.geometry').read_bytes(),
                                                          width,height,row['phase'],row['mode'],report['index_bytes'])))
                for mode in range(10):
                    for suffix in ('images','geometry'):
                        generate.require((output/f'mode-{mode}-frame-0.{suffix}').read_bytes()==(output/f'mode-{mode}-frame-2.{suffix}').read_bytes(),'A/B/A repeat mismatch')
                for frame in range(3):
                    normal=(output/f'mode-0-frame-{frame}.images').read_bytes()
                    for mode in (1,6): generate.require(normal==(output/f'mode-{mode}-frame-{frame}.images').read_bytes(),'draw-order/load mismatch')
                generate.require((output/'mode-0-frame-0.images').read_bytes()!=(output/'mode-0-frame-1.images').read_bytes(),'moving geometry did not change output')
                result=dict(extent=[width,height],device=devices[0],checks=checks,
                            files={p.name:digest(p) for p in sorted(output.iterdir()) if p.is_file()})
                if args.public: result.update(backend=backend,directory=output.name)
                if backend=='public': result['native_comparison']=compare_outputs(dest/f'{width}x{height}-native',output,rows)
                if reference is not None:
                    matches=[r for r in reference['runs'] if r['extent']==[width,height] and r['backend']==backend]
                    generate.require(len(matches)==1 and matches[0]['device']==devices[0]
                        and [dict(mode=c['mode'],phase=c['phase'],frame=c['frame']) for c in matches[0]['checks']]==rows,
                        'UINT32 reference device/frame/extent mismatch')
                    result['index_width_comparison']=compare_index_width(args.reference32.parent,matches[0],output,rows)
                report['runs'].append(result)
                save();print(f'Scene {backend} {width}x{height}: all 30 color/depth/geometry/guard checks PASS',flush=True)
        report['complete']=True;save()
    except Exception as error:
        report['error']=str(error);save();raise
    print(f'{"Matched public/native" if args.public else "Native"} indexed/depth control PASS; no full M4/performance claim: {dest / "report.json"}')


if __name__=='__main__': main()
