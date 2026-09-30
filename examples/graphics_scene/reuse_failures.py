#!/usr/bin/env python3
"""Bounded consumer CPU/GPU cleanup tests; controlled HOST-write rejection."""
import argparse
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
sys.dont_write_bytecode=True
import reuse
ROOT,HERE=reuse.ROOT,reuse.HERE
require,digest=reuse.require,reuse.digest


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reuse-report',type=Path,required=True)
    parser.add_argument('--check',action='store_true')
    args=parser.parse_args()
    old=json.loads(args.reuse_report.read_text())
    require(old.get('complete') and not old.get('dirty') and not old.get('preflight') and not old.get('build_only'),'need accepted reuse report')
    base=args.reuse_report.resolve().parent
    dest=Path(tempfile.mkdtemp(prefix='reuse-failures-',dir=ROOT/'target/graphics-scene'))
    print(f'Failure evidence: {dest}',flush=True)
    report=dict(schema=1,complete=False,revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
                dirty=bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True)),runs=[],
                reuse_report=dict(path=str(args.reuse_report.resolve()),sha256=digest(args.reuse_report)),
                scope='C-consumer allocations and cleanup under HOST-write rejection; not real device loss or LeakSanitizer')
    def save(): (dest/'report.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
    save()
    try:
        sources=[HERE/n for n in ('reuse_public.c','public.c','reuse.h','reuse_cpu_trace.h','reuse_write_failure.c','reuse_failures.py','reuse.py')]
        sources += [ROOT/'include/ogpu.h',ROOT/'examples/learned_image/allocation_tracker.h']
        report['sources']={str(p.relative_to(ROOT)):digest(p) for p in sources}
        compiler=shlex.split(os.getenv('CC','cc'))
        command=[*compiler,'-std=c11','-O1','-g','-Wall','-Wextra','-Werror','-DSCENE_CPU_TRACE',
            '-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(ROOT/'include'),str(HERE/'reuse_public.c'),
            '-L'+str(ROOT/'target/release'),'-Wl,-rpath,'+str(ROOT/'target/release'),'-logpu','-ldl','-o',str(dest/'public')]
        subprocess.run(command,check=True);report['build_command']=command
        command=[*compiler,'-std=c11','-O2','-Wall','-Wextra','-Werror','-shared','-fPIC','-I'+str(ROOT/'include'),
                 str(HERE/'reuse_write_failure.c'),'-ldl','-o',str(dest/'failure.so')]
        subprocess.run(command,check=True);report['shim_command']=command
        report['artifacts']={n:digest(dest/n) for n in ('public','failure.so')}
        report['runtime_sha256']=digest(ROOT/'target/release/libogpu.so')
        require(report['runtime_sha256']==old['runtime_sha256'],'runtime changed since accepted reuse')
        require(digest(base/'trace.so')==old['builds']['trace']['sha256'],'trace changed')
        for stage in ('compute','vertex','fragment'):
            require(digest(base/'shaders16'/(stage+'.spv'))==old['shaders']['16-'+stage]['sha256'],'shader changed')
        if args.check:
            report['complete']=True;report['build_only']=True;save();return
        require(os.getenv('VK_DRIVER_FILES') and 'VK_LAYER_KHRONOS_validation' in os.getenv('VK_INSTANCE_LAYERS','')
            and os.getenv('VK_LAYER_VALIDATE_SYNC')=='1' and not os.getenv('VK_LOADER_LAYERS_DISABLE'),'enable driver/Vulkan/sync validation')
        ref=old['references']['16'];path=Path(ref['path'])
        require(digest(path)==ref['sha256'],'reference report changed')
        _,selected=reuse.reference(path,16,[(257,193)]);ref=selected[(257,193)]
        env=os.environ.copy();env.update(ASAN_OPTIONS='detect_leaks=0',UBSAN_OPTIONS='halt_on_error=1',
            LD_PRELOAD=str(dest/'failure.so'),OGPU_SCENE_TRACE='1',OGPU_TRACE_LOADER=old['environment']['OGPU_TRACE_LOADER'],
            OGPU_VULKAN_LIBRARY=str(base/'trace.so'))
        report['environment']={k:env.get(k) for k in ('VK_DRIVER_FILES','VK_ICD_FILENAMES','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC',
            'ASAN_OPTIONS','UBSAN_OPTIONS','LD_PRELOAD','OGPU_SCENE_TRACE','OGPU_TRACE_LOADER','OGPU_VULKAN_LIBRARY')}
        for strategy in ('reset','replay'):
            for fault in (0,1,3,5,6):
                name=f'{strategy}-{fault}';out=dest/name;out.mkdir();env['OGPU_SCENE_FAIL_WRITE']=str(fault)
                command=[str(dest/'public'),'257','193',str(base/'shaders16'),ref['directory'],'8','8','2',strategy,'16']
                execution=subprocess.run(command,capture_output=True,text=True,env=env,timeout=60)
                stdout,stderr=execution.stdout,execution.stderr
                (out/'stdout').write_text(stdout);(out/'stderr').write_text(stderr)
                require(execution.returncode==(1 if fault else 0),'wrong exit status')
                require(not any(s in stdout+stderr for s in ('Validation Error:','SYNC-HAZARD','AddressSanitizer:','runtime error:')),'sanitizer/validation error')
                require(reuse.rows(stdout,'DEVICE ')==[ref['device']],'wrong device')
                require(reuse.rows(stderr,'WRITE_FAILURE ')==([dict(call=fault)] if fault else []),'wrong injection')
                cpu=reuse.rows(stderr,'CPU_MEMORY_SUMMARY ');gpu=reuse.rows(stderr,'MEMORY_SUMMARY ')
                for samples in (cpu,gpu):
                    require(len(samples)==1 and samples[0]['allocations']==samples[0]['frees']
                        and samples[0]['live_count']==samples[0]['live_bytes']==0,'consumer/native allocation leak')
                require(cpu[0]['allocations']==(10 if fault==1 else 11),'wrong CPU allocation coverage')
                require(gpu[0]['allocations']==(8 if fault==1 else 17 if fault==3 else 18),'wrong GPU allocation coverage')
                counts=reuse.rows(stderr,'COMMAND_COUNTS ')
                require(counts[-1]['phase']==2 and counts[-1]['vkCreateCommandPool']==counts[-1]['vkDestroyCommandPool'],'pool leak')
                expected_submits={0:10,1:0,3:1,5:2,6:3}[fault]
                require(counts[-1]['vkQueueSubmit2']==expected_submits,'fault did not occur at the intended lifecycle point')
                if fault:
                    require(not reuse.rows(stdout,'REUSE_FRAME ') and 'Public scene reuse drained and checked' not in stdout,'failed run accepted output')
                else:
                    reuse.check_run(stdout,stderr,('public',16,8,2,strategy,8,257,193),ref['device'])
                report['runs'].append(dict(name=name,command=command,fault=fault,exit=execution.returncode,cpu=cpu[0],gpu=gpu[0],
                    native_submissions=expected_submits,logs={n:digest(out/n) for n in ('stdout','stderr')}));save()
                print(f'{name}: expected exit, CPU/GPU allocations and pools released',flush=True)
        report['complete']=True;save();print('Consumer cleanup failure matrix PASS; no LeakSanitizer/device-loss claim')
    except Exception as error:
        report['error']=str(error);save();raise


if __name__=='__main__': main()
