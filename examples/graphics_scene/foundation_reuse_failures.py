#!/usr/bin/env python3
"""Foundation two-slot failure cleanup; real waits, no fabricated GPU retirement."""
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
FAULTS=('none','flush1','flush3','flush6','end','submit','unknown','wait','lost')


def check_fault(fault,strategy,code,stdout,stderr):
    require(code==(0 if fault=='none' else 1),'wrong failure status')
    require(not any(x in stdout+stderr for x in ('Validation Error:','SYNC-HAZARD','AddressSanitizer:','runtime error:')),'sanitizer/validation failure')
    injections=[line for line in stderr.splitlines() if line.startswith('INJECT ')]
    require(injections==([] if fault=='none' else ['INJECT '+fault]),'wrong injection')
    if fault!='none':
        require(not reuse.rows(stdout,'REUSE_FRAME ') and 'Public scene reuse drained and checked' not in stdout,'failure accepted output')
    else: require('Public scene reuse drained and checked' in stdout,'missing success marker')
    if fault in ('unknown','lost'):
        require('refusing unsafe teardown' in stderr and 'TEARDOWN ' not in stderr,'unknown completion released operands')
        require(not reuse.rows(stderr,'CPU_MEMORY_SUMMARY ') and not reuse.rows(stderr,'MEMORY_SUMMARY '),'fail-stop unwound resource owners')
        return dict(fail_stop=True)
    cpu=reuse.rows(stderr,'CPU_MEMORY_SUMMARY ');gpu=reuse.rows(stderr,'MEMORY_SUMMARY ')
    for samples in (cpu,gpu):
        require(len(samples)==1 and samples[0]['allocations']>0 and samples[0]['allocations']==samples[0]['frees']
                and samples[0]['live_count']==samples[0]['live_bytes']==0,'allocation leak')
    expected_allocations=8 if fault=='flush1' else 17 if fault=='flush3' else 18
    require(gpu[0]['allocations']==expected_allocations,'wrong allocation lifecycle point')
    counts=reuse.rows(stderr,'COMMAND_COUNTS ')
    require(counts and counts[-1]['phase']==2 and counts[-1]['vkCreateCommandPool']==counts[-1]['vkDestroyCommandPool'],'pool leak')
    submissions={'none':10,'flush1':0,'flush3':1,'flush6':3,'end':2 if strategy=='replay' else 3,'submit':3,'wait':4}[fault]
    require(counts[-1]['vkQueueSubmit2']==submissions and counts[-1]['vkQueueWaitIdle']==0,'wrong submission/drain path')
    return dict(cpu=cpu[0],gpu=gpu[0],submissions=submissions)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reuse-report',required=True,type=Path)
    args=parser.parse_args();old=json.loads(args.reuse_report.read_text());base=args.reuse_report.resolve().parent
    require(old.get('complete') and old.get('api')=='foundation' and not old.get('build_only'),'accepted foundation reuse report required')
    out=Path(tempfile.mkdtemp(prefix='foundation-reuse-failures-',dir=ROOT/'target/graphics-scene'))
    print(f'Foundation failure evidence: {out}',flush=True)
    report=dict(schema=1,complete=False,runs=[],scope='injected consumer cleanup, not real device loss or LeakSanitizer',
        revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        dirty=bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True)),
        reference=dict(path=str(args.reuse_report.resolve()),sha256=digest(args.reuse_report)))
    def save():(out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    save()
    try:
        sources=[HERE/n for n in ('reuse_foundation.c','foundation.c','reuse.h','reuse_cpu_trace.h','reuse_foundation_failures.c',
                                 'foundation_reuse_failures.py','reuse.py','run.py')]
        sources += [ROOT/'include/ogpu_next.h',ROOT/'examples/learned_image/allocation_tracker.h']
        report['sources']={str(p.relative_to(ROOT)):digest(p) for p in sources}
        command=[*shlex.split(os.getenv('CC','cc')),'-std=c11','-O1','-g','-Wall','-Wextra','-Werror',
            '-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(ROOT/'include'),str(HERE/'reuse_foundation_failures.c'),
            '-L'+str(ROOT/'target/release'),'-Wl,-rpath,'+str(ROOT/'target/release'),'-logpu','-ldl','-o',str(out/'consumer')]
        command += ['-Wl,--wrap=ogpu_next_'+n for n in ('memory_flush','commands_end','queue_submit','timeline_wait','arena_destroy','memory_destroy')]
        subprocess.run(command,check=True);report['build']=dict(command=command,sha256=digest(out/'consumer'))
        require(digest(ROOT/'target/release/libogpu.so')==old['runtime_sha256'],'runtime changed')
        require(digest(base/'trace.so')==old['builds']['trace']['sha256'],'trace changed')
        for stage in ('compute','vertex','fragment'):
            require(digest(base/'shaders16'/(stage+'.spv'))==old['shaders']['16-'+stage]['sha256'],'shader changed')
        ref=old['references']['16'];path=Path(ref['path']);require(digest(path)==ref['sha256'],'reference report changed')
        _,selected=reuse.reference(path,16,[(257,193)]);ref=selected[(257,193)]
        env=dict(os.environ,ASAN_OPTIONS='detect_leaks=0',UBSAN_OPTIONS='halt_on_error=1',OGPU_SCENE_TRACE='1',
                 OGPU_TRACE_LOADER=old['environment']['OGPU_TRACE_LOADER'],OGPU_VULKAN_LIBRARY=str(base/'trace.so'))
        require(env.get('VK_DRIVER_FILES') and 'VK_LAYER_KHRONOS_validation' in env.get('VK_INSTANCE_LAYERS','')
                and env.get('VK_LAYER_VALIDATE_SYNC')=='1' and not env.get('VK_LOADER_LAYERS_DISABLE'),'enable selected ICD and sync validation')
        report['environment']={k:env.get(k) for k in ('VK_DRIVER_FILES','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC','OGPU_TRACE_LOADER',
                                                    'OGPU_VULKAN_LIBRARY','ASAN_OPTIONS','UBSAN_OPTIONS')}
        for strategy in ('reset','replay'):
            for fault in FAULTS:
                dest=out/(strategy+'-'+fault);dest.mkdir()
                command=[str(out/'consumer'),fault,'257','193',str(base/'shaders16'),ref['directory'],'8','8','2',strategy,'16']
                p=subprocess.run(command,capture_output=True,text=True,env=env,timeout=60)
                (dest/'stdout').write_text(p.stdout);(dest/'stderr').write_text(p.stderr)
                result=check_fault(fault,strategy,p.returncode,p.stdout,p.stderr)
                require(reuse.rows(p.stdout,'DEVICE ')==[ref['device']],'wrong device')
                require(reuse.rows(p.stdout,'PUBLIC_SCENE ')==[dict(foundation=1,index_bytes=2,gpu_generated=True)],'wrong API profile')
                if fault=='none':reuse.check_run(p.stdout,p.stderr,('public',16,8,2,strategy,8,257,193),ref['device'],True)
                report['runs'].append(dict(strategy=strategy,fault=fault,command=command,**result,
                    logs={p.name:digest(p) for p in dest.iterdir()}));save();print(f'{strategy}/{fault} PASS',flush=True)
        report['complete']=True;save()
    except Exception as error:report['error']=str(error);save();raise


if __name__=='__main__':main()
