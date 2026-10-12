#!/usr/bin/env python3
"""Generate/check the full indexed/depth scene's typed shader interfaces and build."""
import argparse
import os
import shlex
import subprocess
import sys
from pathlib import Path
sys.dont_write_bytecode=True
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
sys.path.insert(0,str(HERE.parent/'compiler'))
import generate as g
import link_graphics


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check',action='store_true')
    parser.add_argument('--api',choices=('installed','foundation'),default='installed')
    parser.add_argument('--build-dir',type=Path,help='isolated output directory for concurrent acceptance runs')
    args=parser.parse_args()
    g.require(not os.getenv('CARGO_TARGET_DIR'),'leave CARGO_TARGET_DIR unset')
    build=args.build_dir or ROOT/'target/compiler-scene';compiler=os.getenv('SLANGC','slangc')
    artifacts={}
    compute=[('scene_prepare','prepare.slang'),('scene_prepare16','prepare16.slang')]
    if args.api=='foundation':compute += [('scene_reuse_prepare','prepare_reuse.slang'),('scene_reuse_prepare16','prepare_reuse16.slang')]
    for name,source in compute:
        path=HERE/source
        reflection,assembly,binary=g.compile_source(path,build/name,compiler)
        artifacts[name]=g.header(reflection,assembly,binary,path.read_bytes(),name,api=args.api)
    artifacts['scene_pair']=link_graphics.header(HERE/'scene.vert.slang',HERE/'scene.frag.slang',build/'pair',compiler,'scene_pair',api=args.api)
    for name,content in artifacts.items():
        path=HERE/(('foundation_' if args.api=='foundation' else '')+name+'.generated.h')
        if args.check:g.require(path.read_text()==content,'stale scene artifact: '+name)
        else:path.write_text(content)
    subprocess.run([sys.executable,'-B',str(HERE/'test_generated_runner.py')],check=True)
    subprocess.run([sys.executable,'-B',str(HERE/'test_oracle.py')],check=True)
    subprocess.run(['cargo','build','--locked','--release','-p','ogpu'],cwd=ROOT,check=True)
    library=ROOT/'target/release'
    subprocess.run([*shlex.split(os.getenv('CC','cc')),'-std=c11','-O2','-Wall','-Wextra','-Werror',
        '-I'+str(ROOT/'include'),'-I'+str(HERE),str(HERE/('foundation_generated.c' if args.api=='foundation' else 'generated.c')),
        '-L'+str(library),'-Wl,-rpath,'+str(library),'-logpu','-o',str(build/('scene-foundation' if args.api=='foundation' else 'scene'))],check=True)
    if args.api=='foundation':
        subprocess.run([*shlex.split(os.getenv('CC','cc')),'-std=c11','-O2','-Wall','-Wextra','-Werror',
            '-I'+str(ROOT/'include'),'-I'+str(HERE),str(HERE/'reuse_foundation_generated.c'),
            '-L'+str(library),'-Wl,-rpath,'+str(library),'-logpu','-ldl','-o',str(build/'reuse-foundation')],check=True)
    print(f'Generated {args.api} indexed/depth scene build PASS; output acceptance is separate')


if __name__=='__main__':main()
