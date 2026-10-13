#!/usr/bin/env python3
"""Opt-in M5 synchronous host-wall observations from an installed SDK consumer."""
import argparse
import json
import math
import os
from pathlib import Path
import subprocess
import tempfile
from run import digest,require,verify_transformer,verify_quantized

HERE=Path(__file__).resolve().parent


def verify_timing(text,kind):
    def rows(prefix):return [dict(field.split('=',1) for field in line.split()[1:])
                             for line in text.splitlines() if line.startswith(prefix+' ')]
    timings=rows('ml_timing');samples=rows('ml_samples')
    keys=('d','h','t','precision') if kind=='transformer' else ('k','m','n','precision')
    shapes=[(1,1,1),(13,29,3),(13,29,17),(64,128,16),(65,129,17)] if kind=='transformer' else [
        (32,1,1),(32,11,3),(64,13,17),(96,17,5),(256,128,64)]
    expected=[(*map(str,s),p) for s in shapes for p in (('f32','f16') if kind=='transformer' else ('f32','f16','q8'))]
    require([tuple(r.get(k) for k in keys) for r in timings]==expected,'incomplete/duplicate timing cases')
    require([tuple(r.get(k) for k in keys) for r in samples]==expected,'incomplete/duplicate timing samples')
    for row,raw in zip(timings,samples):
        require(row.get('kind')==raw.get('kind')==kind and row.get('warmups')=='2' and row.get('samples')=='9'
                and row.get('iterations')=='10','wrong timing protocol')
        values=[float(v) for v in raw['per_graph_ms'].split(',')]
        require(len(values)==9 and all(math.isfinite(v) and v>0 for v in values),'invalid timing samples')
        ordered=sorted(values)
        for name,value in zip(('min_ms','median_ms','max_ms'),(ordered[0],ordered[4],ordered[-1])):
            require(math.isclose(float(row[name]),value,rel_tol=1e-7),'summary/sample mismatch')
        items=int(row['t' if kind=='transformer' else 'n'])
        require(math.isclose(float(row['items_per_s']),items*1000/ordered[4],rel_tol=1e-7),'throughput mismatch')
        require(int(row['allocation_bytes'])>int(row['weight_bytes'])>0,'invalid storage summary')
        row['per_graph_ms']=values
    return timings


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--placement',choices=('host','device','both'),default='both')
    args=parser.parse_args()
    output=Path(tempfile.mkdtemp(prefix='measure-',dir=HERE))
    print(f'M5 measurement evidence: {output}',flush=True)
    report=dict(schema=1,complete=False,runs=[],scope='synchronous host wall costs, not GPU-only/native parity/Metal')
    def save():(output/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    save()
    try:
        require(not os.getenv('VK_INSTANCE_LAYERS') and not os.getenv('VK_LAYER_VALIDATE_SYNC') and
                os.getenv('VK_LOADER_LAYERS_DISABLE')=='VK_LAYER_KHRONOS_validation','disable validation explicitly')
        require(os.getenv('VK_DRIVER_FILES'),'select one driver')
        pkg=os.getenv('PKG_CONFIG','pkg-config')
        def variable(name):return subprocess.check_output([pkg,'--variable='+name,'ogpu-foundation'],text=True).strip()
        require(variable('ogpu_foundation')=='1','foundation revision mismatch')
        report['sdk']=dict(revision=variable('ogpu_revision'),runtime_sha256=digest(Path(variable('libdir'))/'libogpu.so'),
            header_sha256=digest(Path(variable('includedir'))/'ogpu_next.h'))
        report['environment']={k:os.getenv(k) for k in ('VK_DRIVER_FILES','VK_INSTANCE_LAYERS','VK_LAYER_VALIDATE_SYNC','VK_LOADER_LAYERS_DISABLE','OGPU_VULKAN_LIBRARY')}
        report['sources']={str(p.relative_to(HERE)):digest(p) for p in HERE.rglob('*') if p.is_file() and
            (p.parent==HERE and p.suffix in ('.cpp','.h','.py') or p.parent.name=='shaders')}
        save()
        for placement in ('host','device') if args.placement=='both' else (args.placement,):
            for kind,executable,checker in [('transformer','ogpu-ggml-transformer-check',verify_transformer),
                                           ('linear','ogpu-ggml-quantized-check',verify_quantized)]:
                binary=HERE/'build'/executable
                command=[str(binary),str(HERE/'shaders'),'0',placement,'--timing']
                p=subprocess.run(command,text=True,capture_output=True,timeout=300)
                stem=output/(placement+'-'+kind)
                stem.with_suffix('.stdout').write_text(p.stdout);stem.with_suffix('.stderr').write_text(p.stderr)
                require(p.returncode==0,'measurement process failed')
                require(not any(s in p.stdout+p.stderr for s in ('Validation Error:','SYNC-HAZARD','runtime error:','ERROR: AddressSanitizer')),'diagnostic in measurement')
                devices=[line.removeprefix('ML_DEVICE ') for line in p.stdout.splitlines() if line.startswith('ML_DEVICE ')]
                require(len(devices)==1 and devices[0],'missing/duplicate device identity')
                if report['runs']:require(report['runs'][0]['device']==devices[0],'measurement device changed')
                correctness=checker(p.stdout,placement);timings=verify_timing(p.stdout,kind)
                report['runs'].append(dict(kind=kind,placement=placement,device=devices[0],command=command,
                    executable_sha256=digest(binary),correctness=correctness,timings=timings));save()
                print(f'{placement} {kind}: {len(timings)} measured cases PASS',flush=True)
        report['complete']=True;save();print('M5 host-wall observations PASS; not native parity')
    except Exception as error:
        report['error']=str(error);save();raise


if __name__=='__main__':main()
