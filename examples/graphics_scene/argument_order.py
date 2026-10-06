#!/usr/bin/env python3
"""Bounded native diagnostic: does pushing before pipeline binding explain regression?"""
import argparse
import json
import os
from pathlib import Path
import shlex
import subprocess
import tempfile
import argument_reuse as a
import argument_timing as t


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--correctness',type=Path,required=True)
    args=parser.parse_args()
    reference=json.loads(args.correctness.read_text());t.matrix(reference)
    a.require(not reference['dirty'] and 256 in reference['sizes'],'clean 256-byte reference required')
    directory=args.correctness.resolve().parent
    for name,h in reference['references'].items():a.require(a.digest(directory/name)==h,'changed artifact/reference')
    out=Path(tempfile.mkdtemp(prefix='argument-order-',dir=a.ROOT/'target/graphics-scene'))
    print('Argument order diagnostic:',out,flush=True)
    report=dict(schema=1,complete=False,scope=__doc__,revision=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),
        dirty=bool(subprocess.check_output(['git','status','--porcelain'],text=True)),
        reference=dict(path=str(args.correctness.resolve()),sha256=a.digest(args.correctness)),builds={},sources={},checks=[],runs=[])
    def save():(out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    save()
    cc=shlex.split(os.getenv('CC','cc'))
    env=os.environ.copy()
    a.require(env.get('VK_DRIVER_FILES')==reference['environment']['VK_DRIVER_FILES'],'select reference ICD')
    a.require('VK_LAYER_KHRONOS_validation' in env.get('VK_INSTANCE_LAYERS','') and env.get('VK_LAYER_VALIDATE_SYNC')=='1','enable validation')
    a.require(env.get('OGPU_VULKAN_LIBRARY')==reference['environment']['OGPU_TRACE_LOADER'],'select actual reference loader')
    try:
        for order in ('after-bind','before-bind'):
            for timing in (False,True):
                flags=a.flags(cc,256,'native-once',diagnostic=not timing,timing=timing)
                if order=='before-bind':flags+=['-DSCENE_ARGUMENT_PUSH_FIRST']
                source=a.HERE/('range_timing.c' if timing else 'reuse_native.c')
                binary=out/(order+('-timing' if timing else '-check'))
                command=flags+[str(source),'-ldl','-o',str(binary)]
                subprocess.run(command,check=True)
                report['builds'][binary.name]=dict(command=command,sha256=a.digest(binary))
                dependencies=subprocess.check_output([*flags,'-MM',str(source)],text=True)
                for p in shlex.split(dependencies.replace('\\\n','').split(':',1)[1]):report['sources'][str(Path(p).resolve())]=a.digest(Path(p))
            checked=env.copy();checked.update(OGPU_TRACE_LOADER=env['OGPU_VULKAN_LIBRARY'],OGPU_VULKAN_LIBRARY=str(directory/'trace.so'),OGPU_SCENE_TRACE='1')
            a.require(a.digest(directory/'trace.so')==reference['builds']['trace']['sha256'],'changed correctness trace')
            command=[str(out/(order+'-check')),'257','193',str(directory/'256'),str(directory/'256/512'),'16','1','reset','512','single']
            result=subprocess.run(command,env=checked,text=True,capture_output=True,check=True,timeout=300)
            (out/(order+'-check.stdout')).write_text(result.stdout);(out/(order+'-check.stderr')).write_text(result.stderr)
            report['checks'].append(dict(order=order,command=command,**a.check(result.stdout,result.stderr,'native-once',256,512,1,16,reference['device'])));save()
        timed=env.copy();timed.update(VK_INSTANCE_LAYERS='',VK_LAYER_VALIDATE_SYNC='0',VK_LOADER_LAYERS_DISABLE='*')
        a.require(not timed.get('LD_PRELOAD') and not timed.get('OGPU_SCENE_TRACE'),'remove diagnostics for timing')
        report['environment']={k:timed.get(k) for k in ('VK_DRIVER_FILES','VK_ICD_FILENAMES','OGPU_VULKAN_LIBRARY','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC','VK_LOADER_LAYERS_DISABLE')}
        for repeat in range(4):
            order=('after-bind','before-bind') if repeat%2==0 else ('before-bind','after-bind')
            for policy in order:
                command=[str(out/(policy+'-timing')),'257','193',str(directory/'256'),str(directory/'256/512'),'992','1','reset','512','single']
                result=subprocess.run(command,env=timed,text=True,capture_output=True,check=True,timeout=300)
                (out/f'{policy}-{repeat}.stdout').write_text(result.stdout);(out/f'{policy}-{repeat}.stderr').write_text(result.stderr)
                report['runs'].append(dict(order=policy,repeat=repeat,command=command,**t.parse(result.stdout,result.stderr,reference['device'],'native-once',256,512,1,992)));save()
        for p in (Path(__file__),a.HERE/'argument_reuse.py',a.HERE/'argument_timing.py'):report['sources'][str(p.resolve())]=a.digest(p)
        report['logs']={p.name:a.digest(p) for p in out.iterdir() if p.suffix in ('.stdout','.stderr')}
        report['complete']=True;save();print('Order diagnostic complete; not whole-matrix acceptance')
    except Exception as error:report['error']=str(error);save();raise


if __name__=='__main__':main()
