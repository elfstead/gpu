#!/usr/bin/env python3
"""Matched old/new installed-runtime diagnostic; not a general parity measurement."""
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
    parser.add_argument('--old-sdk',type=Path,required=True)
    parser.add_argument('--new-sdk',type=Path,required=True)
    args=parser.parse_args()
    ref=json.loads(args.correctness.read_text());t.matrix(ref)
    a.require(not ref['dirty'] and 256 in ref['sizes'],'clean 256-byte reference required')
    directory=args.correctness.resolve().parent
    for name,h in ref['references'].items():a.require(a.digest(directory/name)==h,'changed shader/output reference')
    out=Path(tempfile.mkdtemp(prefix='argument-runtime-',dir=a.ROOT/'target/graphics-scene'))
    print('Runtime diagnostic:',out,flush=True)
    report=dict(schema=1,complete=False,scope=__doc__,revision=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),
        dirty=bool(subprocess.check_output(['git','status','--porcelain'],text=True)),
        reference=dict(path=str(args.correctness.resolve()),sha256=a.digest(args.correctness)),builds={},sources={},runs=[])
    def save():(out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    save()
    paths=('native-once','old-public','new-public','new-current','workspace-public','workspace-current')
    env=os.environ.copy()
    a.require(env.get('VK_DRIVER_FILES')==ref['environment']['VK_DRIVER_FILES'],'select reference ICD')
    a.require(env.get('OGPU_VULKAN_LIBRARY')==ref['environment']['OGPU_TRACE_LOADER'],'select actual reference loader')
    a.require(not env.get('LD_PRELOAD') and not env.get('OGPU_SCENE_TRACE') and not env.get('LD_LIBRARY_PATH'),'remove library/diagnostic overrides')
    env.update(VK_INSTANCE_LAYERS='',VK_LAYER_VALIDATE_SYNC='0',VK_LOADER_LAYERS_DISABLE='*')
    report['environment']={k:env.get(k) for k in ('VK_DRIVER_FILES','VK_ICD_FILENAMES','OGPU_VULKAN_LIBRARY','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC','VK_LOADER_LAYERS_DISABLE')}
    try:
        for path in paths:
            policy='native-once' if path=='native-once' else 'public-current' if path.endswith('-current') else 'public'
            flags=a.flags(shlex.split(os.getenv('CC','cc')),256,policy,timing=True)
            if path!='native-once':
                prefix=(args.old_sdk if path=='old-public' else args.new_sdk).resolve()
                headers=a.ROOT/'include' if path.startswith('workspace') else prefix/'include'
                library=a.ROOT/'target/release' if path.startswith('workspace') else prefix/'lib'
                flags=[x for x in flags if x!='-I'+str(a.ROOT/'include')]+['-I'+str(headers)]
                link=['-L'+str(library),'-Wl,-rpath,'+str(library),'-logpu']
            else:link=[]
            source=a.HERE/'range_timing.c';command=flags+[str(source),'-ldl',*link,'-o',str(out/path)]
            subprocess.run(command,check=True)
            build=dict(command=command,sha256=a.digest(out/path))
            if path!='native-once':
                build.update(library=str(library),runtime_sha256=a.digest(library/'libogpu.so'),header_sha256=a.digest(headers/'ogpu.h'))
                build['ldd']=subprocess.check_output(['ldd',str(out/path)],env=env,text=True)
                a.require(str(library/'libogpu.so') in build['ldd'],'wrong loaded runtime')
            report['builds'][path]=build
            dependencies=subprocess.check_output([*flags,'-MM',str(source)],text=True)
            for p in shlex.split(dependencies.replace('\\\n','').split(':',1)[1]):report['sources'][str(Path(p).resolve())]=a.digest(Path(p))
        for repeat in range(len(paths)):
            for path in paths[repeat:]+paths[:repeat]:
                command=[str(out/path),'257','193',str(directory/'256'),str(directory/'256/512'),'992','1','reset','512','single']
                result=subprocess.run(command,env=env,text=True,capture_output=True,check=True,timeout=300)
                (out/f'{path}-{repeat}.stdout').write_text(result.stdout);(out/f'{path}-{repeat}.stderr').write_text(result.stderr)
                policy='native-once' if path=='native-once' else 'public-current' if path.endswith('-current') else 'public'
                report['runs'].append(dict(path=path,repeat=repeat,command=command,**t.parse(result.stdout,result.stderr,ref['device'],policy,256,512,1,992)));save()
        report['logs']={p.name:a.digest(p) for p in out.iterdir() if p.suffix in ('.stdout','.stderr')}
        for p in (Path(__file__),a.HERE/'argument_reuse.py',a.HERE/'argument_timing.py'):report['sources'][str(p.resolve())]=a.digest(p)
        report['complete']=True;save();print('Runtime diagnostic complete; final-slot bytes checked')
    except Exception as error:report['error']=str(error);save();raise


if __name__=='__main__':main()
