#!/usr/bin/env python3
"""Direct Vulkan count-then-fixed regression: retain failures, never waive the oracle."""
import argparse
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
sys.dont_write_bytecode = True
import frontier as f


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native', required=True, type=Path, help='accepted clean native identity report')
    args = parser.parse_args()
    reference = json.loads(args.native.read_text())
    f.require(reference.get('complete') and reference.get('identity') and not reference.get('dirty'), 'unclean/incomplete reference')
    directory = args.native.resolve().parent
    out = Path(tempfile.mkdtemp(prefix='count-followup-', dir=f.ROOT/'target/graphics-scene'))
    print(f'Count-followup diagnostic: {out}', flush=True)
    report = dict(schema=1, complete=False, passed=False,
                  revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=f.ROOT,text=True).strip(),
                  dirty=bool(subprocess.check_output(['git','status','--porcelain'],cwd=f.ROOT,text=True)),
                  reference=dict(path=str(args.native.resolve()),sha256=f.digest(args.native)), frames=[])
    def save(): (out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    save()
    try:
        env = os.environ.copy()
        f.require(env.get('VK_DRIVER_FILES') and 'VK_LAYER_KHRONOS_validation' in env.get('VK_INSTANCE_LAYERS','')
                  and env.get('VK_LAYER_VALIDATE_SYNC')=='1' and not env.get('VK_LOADER_LAYERS_DISABLE'), 'enable selected ICD and validation')
        compiler = shlex.split(env.get('CC','cc'))
        report['compiler'] = subprocess.check_output([*compiler,'--version'],text=True).splitlines()[0]
        flags = [*compiler,'-std=c11','-O2','-Wall','-Wextra','-Werror','-I'+str(f.ROOT/'include'),
                 '-I'+str(f.ROOT/'vendor/Vulkan-Headers/include'),'-I'+str(f.ROOT/'examples/learned_image/generated')]
        report['builds'] = {}
        for name, source, extra in [('native','frontier.c',['-DNATIVE_DRAW_IDENTITY']),('trace.so','reuse_trace.c',['-shared','-fPIC'])]:
            command = flags+extra+[str(f.HERE/source),'-ldl','-o',str(out/name)]
            subprocess.run(command,check=True)
            report['builds'][name] = dict(command=command,sha256=f.digest(out/name))
        f.require('ogpu' not in subprocess.check_output(['nm','-u',str(out/'native')],text=True).lower(), 'unexpected runtime linkage')
        sources = list(f.HERE.glob('*.c'))+list(f.HERE.glob('*.h'))+list(f.HERE.glob('*.py'))
        sources += list((f.ROOT/'examples/learned_image').glob('native*'))
        sources += [f.ROOT/'examples/learned_image/allocation_tracker.h',f.ROOT/'examples/learned_image/trace_memory.c']
        sources += list((f.ROOT/'examples/learned_image/generated').glob('*.h'))
        sources += [f.ROOT/'vendor/Vulkan-Headers/include/vulkan/vulkan_core.h',f.ROOT/'include/ogpu.h']
        report['sources'] = {str(p.relative_to(f.ROOT)):f.digest(p) for p in sorted(sources) if p.is_file()}
        report['shaders'] = {}
        for stage, metadata in reference['shaders'].items():
            f.require(f.digest(directory/(stage+'.spv'))==metadata['sha256'], 'changed native shader')
            report['shaders'][stage] = metadata['sha256']
        case = next(c for c in reference['runs'] if c['name']=='257x193-64-count')
        report['device'] = case['device']
        for filename, digest in {**case['files'], **case['logs']}.items():
            f.require(f.digest(directory/case['name']/filename)==digest, 'changed reference artifact')
        env.update(OGPU_TRACE_LOADER=env.get('OGPU_VULKAN_LIBRARY','libvulkan.so.1'),
                   OGPU_VULKAN_LIBRARY=str(out/'trace.so'),OGPU_SCENE_TRACE='1')
        report['environment'] = {k:env.get(k) for k in ('VK_DRIVER_FILES','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC','OGPU_TRACE_LOADER')}
        command = [str(out/'native'),'257','193',str(directory),str(out),'64','count-fixed']
        report['command'] = command
        with (out/'stdout').open('w') as stdout,(out/'stderr').open('w') as stderr:
            execution = subprocess.run(command,env=env,stdout=stdout,stderr=stderr,timeout=300)
        f.require(execution.returncode==0,'native execution failed')
        report['traces'] = f.check_log((out/'stdout').read_text(),(out/'stderr').read_text(),64,'count-fixed',case['device'],True)
        report['logs'] = {n:f.digest(out/n) for n in ('stdout','stderr')}
        for frame in f.frames(64):
            i=frame['frame'];full=0 if frame['phase']==0 else 5
            expected=(directory/case['name']/f'frame-{full}.images').read_bytes()
            actual=(out/f'frame-{i}.images').read_bytes()
            f.require((out/f'frame-{i}.geometry').read_bytes()==f.mesh_expected(frame,'count'), 'producer/count/guard mismatch')
            report['frames'].append(dict(**frame,passed=actual==expected,
                matches_count_only=actual==(directory/case['name']/f'frame-{i}.images').read_bytes(),
                expected_sha256=f.digest(directory/case['name']/f'frame-{full}.images'),
                actual_sha256=f.digest(out/f'frame-{i}.images'),geometry_sha256=f.digest(out/f'frame-{i}.geometry')))
        report['passed']=all(row['passed'] for row in report['frames']);report['complete']=True;save()
        print('Count-followup '+('PASS' if report['passed'] else 'FAIL: fixed draw did not restore full output'),flush=True)
    except Exception as error: report['error']=str(error);save();raise
    return 0 if report['passed'] else 1


if __name__ == '__main__': sys.exit(main())
