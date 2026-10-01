#!/usr/bin/env python3
"""Generate/check and run the standalone fixed/count/draw-identity consumer."""
import argparse
import os
import shlex
import subprocess
import sys
sys.dont_write_bytecode = True
import generate as g
import link_graphics


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='reproduce checked-in artifacts and build without GPU execution')
    args = parser.parse_args()
    g.require(not os.getenv('CARGO_TARGET_DIR'), 'leave CARGO_TARGET_DIR unset')
    build = g.ROOT/'target/compiler-ranges'
    scene = g.ROOT/'examples/graphics_scene'
    compiler = os.getenv('SLANGC', 'slangc')
    pair = link_graphics.header(scene/'identity.vert.slang', scene/'identity.frag.slang', build, compiler, 'identity')
    source = g.HERE/'range_count.slang'
    reflection, assembly, binary = g.compile_source(source, build/'compute', compiler)
    compute = g.header(reflection, assembly, binary, source.read_bytes(), 'range_count')
    for name, content in (('identity', pair), ('range_count', compute)):
        output = g.HERE/(name+'.generated.h')
        if args.check: g.require(output.read_text() == content, 'stale '+name+' artifact')
        else: output.write_text(content)
    subprocess.run([sys.executable, '-B', str(g.HERE/'test_ranges.py')], check=True)
    subprocess.run([os.getenv('CARGO', 'cargo'), 'build', '--locked', '--release', '-p', 'ogpu'], cwd=g.ROOT, check=True)
    library = g.ROOT/'target/release'
    executable = build/'ranges'
    subprocess.run([*shlex.split(os.getenv('CC', 'cc')), '-std=c11', '-O2', '-Wall', '-Wextra', '-Werror',
                    '-I'+str(g.HERE), '-I'+str(g.ROOT/'include'), str(g.HERE/'ranges.c'),
                    '-L'+str(library), '-Wl,-rpath,'+str(library), '-logpu', '-o', str(executable)], check=True)
    if not args.check:
        result = subprocess.run([str(executable)], capture_output=True, text=True)
        print(result.stdout, end='');print(result.stderr, end='', file=sys.stderr)
        g.require(result.returncode == 0 and 'Validation Error:' not in result.stdout+result.stderr
                  and 'Generated indirect range consumer PASS: 540 frames per device' in result.stdout,
                  'range execution failed')
    print('Generated range '+('build' if args.check else 'execution')+' PASS')


if __name__ == '__main__': main()
