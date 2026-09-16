#!/usr/bin/env python3
"""Reproduce/check generated artifacts, build, and optionally execute both layouts."""
import argparse
import os
from pathlib import Path
import shlex
import subprocess
import sys
sys.dont_write_bytecode = True
import generate


def checked(*args, **kwargs):
    subprocess.run(args, check=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="compile/check but do not execute GPU work")
    args = parser.parse_args()
    os.chdir(generate.ROOT)
    generate.require(not os.getenv("CARGO_TARGET_DIR"), "leave CARGO_TARGET_DIR unset for this runner")
    checked(sys.executable, str(generate.HERE / "generate.py"), *(["--check"] if args.check else []))
    checked(sys.executable, "-B", str(generate.HERE / "test_generate.py"))
    build = generate.ROOT / "target/compiler-workflow"
    # A schema change test, not hand-maintained alternative host declarations.
    source = (generate.HERE / "transform.slang").read_text()
    altered = source.replace("    uint* data;\n    uint count;", "    uint count;\n    uint* data;")
    altered = altered.replace("numthreads(64, 1, 1)", "numthreads(32, 1, 1)")
    generate.require(altered != source, "mutation fixture did not change source")
    variant = build / "reordered"
    variant.mkdir(exist_ok=True)
    variant_source = variant / "transform.slang"
    variant_source.write_text(altered)
    reflected, assembly, binary = generate.compile_source(variant_source, variant, os.getenv("SLANGC", "slangc"))
    (variant / "transform.generated.h").write_text(generate.header(reflected, assembly, binary, altered.encode()))
    checked(os.getenv("CARGO", "cargo"), "build", "--locked", "--release", "-p", "ogpu")
    library = generate.ROOT / "target/release"
    environment = os.environ.copy()
    key = "DYLD_LIBRARY_PATH" if sys.platform == "darwin" else "LD_LIBRARY_PATH"
    environment[key] = str(library) + (os.pathsep + environment[key] if environment.get(key) else "")
    for name, headers in [("original", generate.HERE), ("reordered", variant)]:
        executable = build / f"consumer-{name}"
        checked(*shlex.split(os.getenv("CC", "cc")), "-std=c11", "-O2", "-DNDEBUG",
                "-Wall", "-Wextra", "-Werror", "-I", str(headers), "-I", "include",
                str(generate.HERE / "consumer.c"), "-L", str(library),
                f"-Wl,-rpath,{library}", "-logpu", "-o", str(executable))
        if not args.check:
            output = subprocess.run([str(executable)], check=True, text=True, capture_output=True, env=environment)
            print(output.stdout, end="")
            print(output.stderr, end="", file=sys.stderr)
            generate.require("Validation Error:" not in output.stdout + output.stderr, "Vulkan validation error")
    print("Compiler workflow build/check PASS" if args.check else "Compiler workflow execution PASS")


if __name__ == "__main__":
    main()
