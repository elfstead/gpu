#!/usr/bin/env python3
"""Generate/check source-only foundation records and run two layouts of one caller."""
import argparse
import os
import shlex
import subprocess
import sys
sys.dont_write_bytecode = True
import generate


def checked(*args, **kwargs):
    subprocess.run(args, check=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="reproduce and compile without GPU execution")
    args = parser.parse_args()
    os.chdir(generate.ROOT)
    generate.require(not os.getenv("CARGO_TARGET_DIR"), "leave CARGO_TARGET_DIR unset for this runner")
    build = generate.ROOT / "target/foundation-compiler"
    source = generate.HERE / "transform.slang"
    checked(sys.executable, "-B", str(generate.HERE / "generate.py"), "--api", "foundation",
            "--source", str(source), "--output", str(generate.HERE / "foundation.generated.h"),
            "--build-dir", str(build / "original"), *(["--check"] if args.check else []))
    checked(sys.executable, "-B", str(generate.HERE / "test_foundation.py"))
    original = source.read_text()
    altered = original.replace("    uint* data;\n    uint count;", "    uint count;\n    uint* data;")
    altered = altered.replace("numthreads(64, 1, 1)", "numthreads(32, 1, 1)")
    generate.require(altered != original, "mutation did not change source")
    variant = build / "reordered"
    variant.mkdir(parents=True, exist_ok=True)
    variant_source = variant / "transform.slang"
    variant_source.write_text(altered)
    reflection, assembly, binary = generate.compile_source(variant_source, variant, os.getenv("SLANGC", "slangc"))
    (variant / "foundation.generated.h").write_text(generate.header(reflection, assembly, binary, altered.encode(), api="foundation"))
    checked(os.getenv("CARGO", "cargo"), "build", "--locked", "-p", "ogpu")
    library = generate.ROOT / "target/debug"
    environment = os.environ.copy()
    environment["LD_LIBRARY_PATH"] = str(library) + (os.pathsep + environment["LD_LIBRARY_PATH"] if environment.get("LD_LIBRARY_PATH") else "")
    for name, headers in [("original", generate.HERE), ("reordered", variant)]:
        executable = build / f"consumer-{name}"
        checked(*shlex.split(os.getenv("CC", "cc")), "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-pedantic",
                "-I", str(headers), "-I", "include", str(generate.HERE / "foundation.c"),
                "-L", str(library), f"-Wl,-rpath,{library}", "-logpu", "-o", str(executable))
        if not args.check:
            output = subprocess.run([str(executable)], check=True, text=True, capture_output=True, env=environment)
            print(output.stdout, end="")
            print(output.stderr, end="", file=sys.stderr)
            generate.require("Validation Error:" not in output.stdout + output.stderr, "Vulkan validation error")
    print("Foundation compiler build/check PASS" if args.check else "Foundation compiler execution PASS")


if __name__ == "__main__":
    main()
