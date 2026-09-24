#!/usr/bin/env python3
"""M2 aggregate fixture: root and pointee mutations with unchanged C source."""
import argparse
import os
import shlex
import subprocess
import sys
sys.dont_write_bytecode = True
import generate as g


def mutated(source):
    replacements = [
        ("    float2 scaleBias;\n    uint selectors[2];", "    uint selectors[2];\n    float2 scaleBias;"),
        ("    float* data;\n    Coefficients coefficients[2];\n    uint count;\n    uint flags;",
         "    uint count;\n    uint flags;\n    float* data;\n    Coefficients coefficients[2];"),
        ("    float2 gainBias;\n    uint mapping[2];", "    uint mapping[2];\n    float2 gainBias;"),
        ("    Block* blocks;\n    Controls controls;", "    Controls controls;\n    Block* blocks;"),
        ("numthreads(64, 1, 1)", "numthreads(32, 1, 1)"),
    ]
    for old, new in replacements:
        g.require(source.count(old) == 1, "missing aggregate mutation anchor")
        source = source.replace(old, new)
    return source


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    os.chdir(g.ROOT)
    g.require(not os.getenv("CARGO_TARGET_DIR"), "leave CARGO_TARGET_DIR unset")
    build = g.ROOT / "target/compiler-structured"
    source = g.HERE / "structured.slang"
    original = source.read_text()
    compiler = os.getenv("SLANGC", "slangc")
    variants = ("original", "reordered", "vector3", "vector4")
    for name in variants:
        folder = build if name == "original" else build / name
        folder.mkdir(parents=True, exist_ok=True)
        text = mutated(original) if name == "reordered" else original
        if name.startswith("vector"):
            # Wider vectors need native block alignment as well as C layout.
            # Place Controls first; do not enable scalarBlockLayout implicitly.
            text = text.replace("float2 gainBias", "float"+name[-1]+" gainBias")
            text = text.replace("    uint mapping[2];", "    uint mapping[2];\n    uint reserved["+str(6-int(name[-1]))+"];")
            text = text.replace("    Block* blocks;\n    Controls controls;", "    Controls controls;\n    Block* blocks;")
        shader = source if name == "original" else folder / "structured.slang"
        if name != "original": shader.write_text(text)
        reflection, assembly, binary = g.compile_source(shader, folder, compiler)
        header = g.header(reflection, assembly, binary, text.encode(), "structured")
        output = (g.HERE if name == "original" else folder) / "structured.generated.h"
        if args.check and name == "original": g.require(output.read_text() == header, "stale structured header")
        else: output.write_text(header)
    subprocess.run([sys.executable, "-B", str(g.HERE / "test_structured.py")], check=True)
    subprocess.run([os.getenv("CARGO", "cargo"), "build", "--locked", "--release", "-p", "ogpu"], check=True)
    library = g.ROOT / "target/release"
    env = os.environ.copy(); env["LD_LIBRARY_PATH"] = str(library)+os.pathsep+env.get("LD_LIBRARY_PATH", "")
    for name in variants:
        headers = g.HERE if name == "original" else build / name
        executable = build / ("structured-" + name)
        subprocess.run([*shlex.split(os.getenv("CC", "cc")), "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror",
                        "-I", str(headers), "-Iinclude", str(g.HERE / "structured.c"), "-L", str(library),
                        f"-Wl,-rpath,{library}", "-logpu", "-o", str(executable)], check=True)
        if not args.check:
            result = subprocess.run([str(executable)], capture_output=True, text=True, env=env)
            print(result.stdout, end=""); print(result.stderr, end="", file=sys.stderr)
            g.require(result.returncode == 0 and "Validation Error:" not in result.stdout+result.stderr
                      and "Structured PASS:" in result.stdout, "structured execution failed")
    print("Structured original/reordered/vector3/vector4 " + ("build" if args.check else "execution") + " PASS")


if __name__ == "__main__": main()
