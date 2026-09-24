#!/usr/bin/env python3
"""M2 scalar-root check: generated FP32 fields, mutated layout, unchanged C consumer."""
import argparse
import os
from pathlib import Path
import shlex
import subprocess
import sys
sys.dont_write_bytecode = True
import generate as g


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="reproduce headers/build, without GPU execution")
    args = parser.parse_args()
    os.chdir(g.ROOT)
    g.require(not os.getenv("CARGO_TARGET_DIR"), "leave CARGO_TARGET_DIR unset")
    build = g.ROOT / "target/compiler-affine"
    source = g.HERE / "affine.slang"
    original = source.read_text()
    altered = original.replace("    float* data;\n    uint count;\n    float scale;\n    float bias;",
                               "    float bias;\n    float scale;\n    uint count;\n    float* data;")
    altered = altered.replace("numthreads(64, 1, 1)", "numthreads(32, 1, 1)")
    g.require(altered != original, "mutation unchanged")
    compiler = os.getenv("SLANGC", "slangc")
    for name in ("original", "reordered"):
        folder = build if name == "original" else build / name
        folder.mkdir(parents=True, exist_ok=True)
        text = original if name == "original" else altered
        shader = source if name == "original" else folder / "affine.slang"
        if name != "original": shader.write_text(text)
        reflection, assembly, binary = g.compile_source(shader, folder, compiler)
        header = g.header(reflection, assembly, binary, text.encode(), "affine")
        output = (g.HERE if name == "original" else folder) / "affine.generated.h"
        if args.check and name == "original":
            g.require(output.read_text() == header, "stale affine header")
        else: output.write_text(header)
    subprocess.run([sys.executable, "-B", str(g.HERE / "test_affine.py")], check=True)
    subprocess.run([os.getenv("CARGO", "cargo"), "build", "--locked", "--release", "-p", "ogpu"], check=True)
    env = os.environ.copy()
    library = g.ROOT / "target/release"
    env["LD_LIBRARY_PATH"] = str(library) + os.pathsep + env.get("LD_LIBRARY_PATH", "")
    for name, headers in (("original", g.HERE), ("reordered", build / "reordered")):
        executable = build / ("affine-" + name)
        subprocess.run([*shlex.split(os.getenv("CC", "cc")), "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror",
                        "-I", str(headers), "-Iinclude", str(g.HERE / "affine.c"), "-L", str(library),
                        f"-Wl,-rpath,{library}", "-logpu", "-o", str(executable)], check=True)
        if not args.check:
            result = subprocess.run([str(executable)], capture_output=True, text=True, env=env)
            print(result.stdout, end=""); print(result.stderr, end="", file=sys.stderr)
            g.require(result.returncode == 0 and "Validation Error:" not in result.stdout + result.stderr
                      and "Affine PASS:" in result.stdout, "affine execution failed")
    print("Affine layout/mutation " + ("build" if args.check else "execution") + " PASS")


if __name__ == "__main__": main()
