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
    args=parser.parse_args()
    g.require(not os.getenv('CARGO_TARGET_DIR'),'leave CARGO_TARGET_DIR unset')
    build=ROOT/'target/compiler-scene';compiler=os.getenv('SLANGC','slangc')
    artifacts={}
    for name,source in (('scene_prepare','prepare.slang'),('scene_prepare16','prepare16.slang')):
        path=HERE/source
        reflection,assembly,binary=g.compile_source(path,build/name,compiler)
        artifacts[name]=g.header(reflection,assembly,binary,path.read_bytes(),name)
    artifacts['scene_pair']=link_graphics.header(HERE/'scene.vert.slang',HERE/'scene.frag.slang',build/'pair',compiler,'scene_pair')
    for name,content in artifacts.items():
        path=HERE/(name+'.generated.h')
        if args.check:g.require(path.read_text()==content,'stale scene artifact: '+name)
        else:path.write_text(content)
    subprocess.run([sys.executable,'-B',str(HERE/'test_generated_runner.py')],check=True)
    subprocess.run([sys.executable,'-B',str(HERE/'test_oracle.py')],check=True)
    subprocess.run(['cargo','build','--locked','--release','-p','ogpu'],cwd=ROOT,check=True)
    library=ROOT/'target/release'
    subprocess.run([*shlex.split(os.getenv('CC','cc')),'-std=c11','-O2','-Wall','-Wextra','-Werror',
        '-I'+str(ROOT/'include'),str(HERE/'generated.c'),'-L'+str(library),'-Wl,-rpath,'+str(library),
        '-logpu','-o',str(build/'scene')],check=True)
    print('Generated indexed/depth scene build PASS; execute the installed runner for full output checks')


if __name__=='__main__':main()
