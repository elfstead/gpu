#!/usr/bin/env python3
"""Generated heap roots: existing pixel/lifetime oracle, original and reordered."""
import argparse
import os
import re
import shlex
import subprocess
import sys
sys.dont_write_bytecode = True
import generate as g

SHADERS = (("heap-process", "compute", "comp"), ("heap-sample", "fragment", "frag"))


def mutate(source):
    source, count = re.subn(r"struct Root \{([^{}]+)\};",
        lambda m: "struct Root { "+" ".join(field.strip()+";" for field in
            reversed(m[1].split(";")[:-1]))+" };", source)
    g.require(count == 1, "missing heap root mutation anchor")
    return source.replace("numthreads(64, 1, 1)", "numthreads(32, 1, 1)")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="check generated files and build, without GPU execution")
    args = parser.parse_args()
    os.chdir(g.ROOT)
    g.require(not os.getenv("CARGO_TARGET_DIR"), "leave CARGO_TARGET_DIR unset")
    build = g.ROOT / "target/compiler-heaps"
    compiler = os.getenv("SLANGC", "slangc")
    for variant in ("original", "reordered"):
        for name, stage, suffix in SHADERS:
            folder = build / variant / name
            folder.mkdir(parents=True, exist_ok=True)
            source = g.ROOT / "examples/shaders" / (name+".slang")
            text = source.read_text()
            if variant == "reordered":
                source = folder / (name+".slang")
                text = mutate(text)
                source.write_text(text)
            reflection, assembly, binary = g.compile_source(source, folder, compiler, stage, native_heaps=True)
            generated = g.header(reflection, assembly, binary, text.encode(), name.replace("-", "_"))
            if variant == "original":
                # This migration must not alter the original device artifact.
                g.require(binary == (g.ROOT / "examples/shaders" / f"{name}.{suffix}.spv").read_bytes(),
                          "original heap device artifact changed")
            header = (g.HERE if variant == "original" else build / variant) / (name.replace("-", "_")+".generated.h")
            if args.check and variant == "original":
                g.require(header.read_text() == generated, "stale heap header")
            else: header.write_text(generated)
    subprocess.run([sys.executable, "-B", str(g.HERE / "test_heaps.py")], check=True)
    subprocess.run([os.getenv("CARGO", "cargo"), "build", "--locked", "--release", "-p", "ogpu"], check=True)
    library = g.ROOT / "target/release"
    env = os.environ.copy(); env["LD_LIBRARY_PATH"] = str(library)+os.pathsep+env.get("LD_LIBRARY_PATH", "")
    for variant in ("original", "reordered"):
        headers = g.HERE if variant == "original" else build / variant
        executable = build / ("heap-"+variant)
        subprocess.run([*shlex.split(os.getenv("CC", "cc")), "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror",
                        "-I", str(headers), "-Iinclude", "examples/heap_image.c", "-L", str(library),
                        f"-Wl,-rpath,{library}", "-logpu", "-o", str(executable)], check=True)
        if not args.check:
            result = subprocess.run([str(executable), "examples/shaders/fullscreen.vert.spv",
                                     "examples/shaders/image-pattern.frag.spv"], capture_output=True, text=True, env=env)
            print(result.stdout, end=""); print(result.stderr, end="", file=sys.stderr)
            g.require(result.returncode == 0 and "Validation Error:" not in result.stdout+result.stderr
                      and result.stdout.count("PASS") >= 6, "heap execution failed")
    print("Generated heap original/reordered " + ("build" if args.check else "execution") + " PASS")


if __name__ == "__main__": main()
