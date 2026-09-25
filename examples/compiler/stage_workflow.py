#!/usr/bin/env python3
"""Bounded stage linking with original/reordered/renamed/noperspective pairs."""
import argparse
import os
import shlex
import subprocess
import sys
sys.dont_write_bytecode = True
import generate as g
import link_graphics

VARIANTS = ("original", "reordered", "renamed", "noperspective")


def mutate(text, variant, stage):
    if variant == "reordered":
        old = "    float2 uv : TEXCOORD0;\n    nointerpolation uint tag : TEXCOORD1;"
        g.require(old in text, "stage mutation anchor missing")
        text = text.replace(old, "    nointerpolation uint tag : TEXCOORD1;\n    float2 uv : TEXCOORD0;")
        text = text.replace("uint width; uint height;", "uint height; uint width;")
    elif variant == "renamed" and stage == "fragment":
        text = text.replace("float2 uv :", "float2 coordinates :").replace("input.uv", "input.coordinates")
        text = text.replace("uint tag :", "uint marker :").replace("input.tag", "input.marker")
    elif variant == "noperspective" and stage == "fragment":
        text = text.replace("    float2 uv :", "    noperspective float2 uv :")
    return text


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    os.chdir(g.ROOT)
    g.require(not os.getenv("CARGO_TARGET_DIR"), "leave CARGO_TARGET_DIR unset")
    build = g.ROOT / "target/compiler-stages"
    for variant in VARIANTS:
        folder = build / variant
        folder.mkdir(parents=True, exist_ok=True)
        sources = {}
        for stage in ("vertex", "fragment"):
            original = g.HERE / f"stage_{stage}.slang"
            sources[stage] = original if variant == "original" else folder / original.name
            if variant != "original": sources[stage].write_text(mutate(original.read_text(), variant, stage))
        content = link_graphics.header(sources["vertex"], sources["fragment"], folder,
                                       os.getenv("SLANGC", "slangc"), "pattern")
        output = (g.HERE if variant == "original" else folder) / "pattern.generated.h"
        if args.check and variant == "original": g.require(output.read_text() == content, "stale pattern header")
        else: output.write_text(content)
    subprocess.run([sys.executable, "-B", str(g.HERE / "test_stages.py")], check=True)
    subprocess.run([os.getenv("CARGO", "cargo"), "build", "--locked", "--release", "-p", "ogpu"], check=True)
    library = g.ROOT / "target/release"
    env = os.environ.copy(); env["LD_LIBRARY_PATH"] = str(library)+os.pathsep+env.get("LD_LIBRARY_PATH", "")
    for variant in VARIANTS:
        headers = g.HERE if variant == "original" else build / variant
        executable = build / ("stage-"+variant)
        subprocess.run([*shlex.split(os.getenv("CC", "cc")), "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror",
                        "-I", str(headers), "-Iinclude", str(g.HERE / "stage_pair.c"), "-L", str(library),
                        f"-Wl,-rpath,{library}", "-logpu", "-o", str(executable)], check=True)
        if not args.check:
            result = subprocess.run([str(executable)], capture_output=True, text=True, env=env)
            print(result.stdout, end=""); print(result.stderr, end="", file=sys.stderr)
            g.require(result.returncode == 0 and "Validation Error:" not in result.stdout+result.stderr
                      and result.stdout.count("PASS") >= 6, "stage execution failed")
    print("Graphics pair original/reordered/renamed/noperspective " + ("build" if args.check else "execution") + " PASS")


if __name__ == "__main__": main()
