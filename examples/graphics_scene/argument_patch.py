#!/usr/bin/env python3
"""Four-byte native updates versus full roots: correctness/expressibility, not timing."""
import argparse
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
sys.dont_write_bytecode=True
import argument_snapshot as s
ROOT,HERE=s.ROOT,s.HERE
require,digest=s.require,s.digest
PATHS=('native-full','native-partial','public')


def updates(path,size):
    require(path in PATHS and size in (64,256),'invalid partial update case')
    result=[]
    for frame in range(4):
        result.extend([dict(offset=0,bytes=32),dict(offset=0,bytes=size)])
        if path=='native-full' or (path=='public' and frame==2):result.append(dict(offset=0,bytes=size))
        elif path=='native-partial' and frame==2:result.append(dict(offset=size-4,bytes=4))
    return result


def check_trace(stdout,stderr,path,size):
    require(s.f.rows(stdout,'ARGUMENT_PATCH_POLICY ')==[dict(single_word=True,native_partial=path=='native-partial')],'wrong patch policy')
    wanted=updates(path,size);require(s.f.rows(stderr,'ARGUMENT_PUSH ')==wanted,'wrong actual native update ranges')
    commands=s.f.rows(stderr,'COMMAND_COUNTS ')
    require(len(commands)==1 and commands[0]['phase']==2,'missing final command accounting')
    c=commands[0]
    require(c['vkCmdPushDataEXT']==len(wanted) and c['vkCmdDrawIndexedIndirect2KHR']==c['indexed_records']==8
            and c['vkCmdDrawIndexedIndirectCount2KHR']==c['counted_capacity']==c['vkQueueWaitIdle']==0
            and c['vkCmdBeginRendering']==c['vkCmdEndRendering']==4,'wrong draw/scope/idle accounting')
    memory=s.f.rows(stderr,'MEMORY_SUMMARY ')
    require(len(memory)==1 and memory[0]['allocations']==memory[0]['frees']==memory[0]['peak_count']==10
            and memory[0]['live_count']==memory[0]['live_bytes']==0,'allocation lifetime mismatch')
    allocations=s.f.rows(stderr,'ALLOCATE ');require(len(allocations)==10,'missing allocation trace')
    return dict(updates=wanted,memory=memory[0],allocation_shape=sorted((a['bytes'],a['type']) for a in allocations),commands=c)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--snapshots',required=True,type=Path)
    parser.add_argument('--build-only',action='store_true')
    args=parser.parse_args()
    out=Path(tempfile.mkdtemp(prefix='argument-patch-',dir=ROOT/'target/graphics-scene'));print(f'Argument patch evidence: {out}',flush=True)
    report=dict(schema=1,complete=False,build_only=args.build_only,scope=__doc__,runs=[],builds={},sources={},
        revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        dirty=bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True)))
    def save():(out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    save()
    try:
        reference=json.loads(args.snapshots.read_text())
        require(reference.get('complete') and not reference.get('dirty') and not reference.get('build_only'),'clean snapshot reference required')
        require(reference['sources'][str(HERE/'argument.vert.slang')]==digest(HERE/'argument.vert.slang'),'changed observable shader')
        report['reference']=dict(path=str(args.snapshots.resolve()),sha256=digest(args.snapshots))
        sizes=[size for size in (64,256) if any(r['bytes']==size for r in reference['runs'])]
        report['unsupported']=[n for n in (64,256) if n not in sizes];require(sizes,'no supported scalar-payload root')
        require(not os.getenv('CARGO_TARGET_DIR'),'leave CARGO_TARGET_DIR unset')
        subprocess.run(['cargo','build','--locked','--release','-p','ogpu'],cwd=ROOT,check=True)
        report['runtime_sha256']=digest(ROOT/'target/release/libogpu.so')
        cc=shlex.split(os.getenv('CC','cc'));report['compiler']=subprocess.check_output([*cc,'--version'],text=True).splitlines()[0]
        def dependencies(flags,source):
            deps=subprocess.check_output([*flags,'-MM',str(source)],text=True)
            for p in shlex.split(deps.replace('\\\n','').split(':',1)[1]):report['sources'][str(Path(p).resolve())]=digest(Path(p))
        flags=[*cc,'-std=c11','-O2','-Wall','-Wextra','-Werror','-DSCENE_ARGUMENT_TRACE','-DSCENE_ARGUMENT_PATCH_TRACE',
               '-I'+str(ROOT/'vendor/Vulkan-Headers/include')]
        command=flags+['-shared','-fPIC',str(HERE/'reuse_trace.c'),'-ldl','-o',str(out/'trace.so')]
        subprocess.run(command,check=True);dependencies(flags,HERE/'reuse_trace.c')
        report['builds']['trace']=dict(command=command,sha256=digest(out/'trace.so'))
        env=os.environ.copy()
        if not args.build_only:
            require(env.get('VK_DRIVER_FILES') and 'VK_LAYER_KHRONOS_validation' in env.get('VK_INSTANCE_LAYERS','')
                    and env.get('VK_LAYER_VALIDATE_SYNC')=='1' and not env.get('VK_LOADER_LAYERS_DISABLE'),'select ICD and validation/sync')
        env.update(OGPU_TRACE_LOADER=env.get('OGPU_VULKAN_LIBRARY','libvulkan.so.1'),OGPU_VULKAN_LIBRARY=str(out/'trace.so'))
        report['environment']={k:env.get(k) for k in ('VK_DRIVER_FILES','VK_ICD_FILENAMES','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC','OGPU_TRACE_LOADER')}
        report['shaders']={}
        for size in sizes:
            shaders=args.snapshots.resolve().parent/str(size)
            report['shaders'][str(size)]={}
            for stage in ('compute','vertex','fragment'):
                path=shaders/(stage+'.spv');h=reference['shaders'][str(size)][stage]['sha256']
                require(digest(path)==h,'changed reference artifact');report['shaders'][str(size)][stage]=h
            results=[];budgets=[]
            for path in PATHS:
                folder=out/f'{size}-{path}';folder.mkdir()
                flags=[*cc,'-std=c11','-O2','-Wall','-Wextra','-Werror','-DARGUMENT_PATCH_PAYLOAD',
                       f'-DSCENE_RASTER_BYTES={size}','-I'+str(ROOT/'include')]
                if path!='public':flags+=['-DARGUMENT_NATIVE','-I'+str(ROOT/'vendor/Vulkan-Headers/include'),'-I'+str(ROOT/'examples/learned_image/generated')]
                if path=='native-partial':flags+=['-DARGUMENT_NATIVE_PARTIAL']
                command=flags+[str(HERE/'argument_snapshot.c'),'-ldl']
                if path=='public':command+=['-L'+str(ROOT/'target/release'),'-Wl,-rpath,'+str(ROOT/'target/release'),'-logpu']
                command+=['-o',str(folder/'probe')];subprocess.run(command,check=True);dependencies(flags,HERE/'argument_snapshot.c')
                symbols=subprocess.check_output(['nm','-u',str(folder/'probe')],text=True)
                require('ogpu' not in symbols.lower() if path!='public' else 'ogpu_batch_draw_indexed_indirect' in symbols,'wrong linkage')
                report['builds'][folder.name]=dict(command=command,sha256=digest(folder/'probe'))
                if args.build_only:continue
                command=[str(folder/'probe'),str(shaders),str(folder)]
                with (folder/'stdout').open('w') as stdout,(folder/'stderr').open('w') as stderr:
                    result=subprocess.run(command,env=env,stdout=stdout,stderr=stderr,timeout=300)
                require(result.returncode==0,'partial-update execution failed: '+str(folder))
                stdout,stderr=(folder/'stdout').read_text(),(folder/'stderr').read_text()
                limit,device=s.check_log(stdout,stderr,'public' if path=='public' else 'native',size)
                matches=[r for r in reference['runs'] if r['bytes']==size and r['backend']==('public' if path=='public' else 'native')]
                require(len(matches)==1 and matches[0]['device']==device and matches[0]['limit']==limit,'reference device/limit mismatch')
                data=(folder/'snapshots.images').read_bytes();checks=s.check(data,size,single_word=True)
                require((folder/'final.geometry').read_bytes()==s.oracle.geometry(0,False)+bytes([0xa5])*4,'geometry/guard mismatch')
                traced=check_trace(stdout,stderr,path,size);results.append(data);budgets.append(traced)
                report['runs'].append(dict(bytes=size,path=path,command=command,device=device,limit=limit,checks=checks,**traced,
                    files={p.name:digest(p) for p in folder.iterdir() if p.is_file()}));save()
            if not args.build_only:
                require(results[0]==results[1]==results[2],'native full/partial/public output mismatch')
                require(all(b['memory']==budgets[0]['memory'] and b['allocation_shape']==budgets[0]['allocation_shape'] for b in budgets),'allocation budget mismatch')
                print(f'{size}-byte roots: full/4-byte partial/public output, update-range and allocation checks PASS',flush=True)
        for p in (Path(__file__),HERE/'argument_snapshot.py',HERE/'oracle.py'):report['sources'][str(p.resolve())]=digest(p)
        report['complete']=True;save();print('Argument partial-update '+('BUILD ONLY' if args.build_only else 'PASS; not timing'))
    except Exception as error:report['error']=str(error);save();raise


if __name__=='__main__':main()
