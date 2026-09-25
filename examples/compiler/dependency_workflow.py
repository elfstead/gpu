#!/usr/bin/env python3
"""Nested include/import build receipts, changed layout/local size, same C consumer."""
import argparse
import os
import shutil
import shlex
import subprocess
import sys
sys.dont_write_bytecode = True
import generate as g


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    os.chdir(g.ROOT)
    g.require(not os.getenv("CARGO_TARGET_DIR"), "leave CARGO_TARGET_DIR unset")
    build = g.ROOT / "target/compiler-dependencies"
    source = g.HERE / "dependencies"
    for variant in ("original", "reordered"):
        directory = source if variant == "original" else build / "changed source tree"
        if variant == "reordered":
            shutil.copytree(source, directory, dirs_exist_ok=True)
            root = directory / "include/arguments.slangh"
            old = "    float* data;\n    uint count;\n    float scale;\n    float bias;"
            g.require(old in root.read_text(), "missing included-root mutation anchor")
            root.write_text(root.read_text().replace(old, "    float bias;\n    float scale;\n    uint count;\n    float* data;"))
            config = directory / "include/config.slangh"
            config.write_text(config.read_text().replace("LOCAL_SIZE 64", "LOCAL_SIZE 32"))
        command = [sys.executable, "-B", str(g.HERE / "generate.py"), "--source", str(directory / "affine.slang"),
                   "--build-dir", str(build / variant), "--name", "affine", "--output", str(directory / "affine.generated.h"),
                   "--source-root", str(directory), "--manifest", str(directory / "build.json")]
        if args.check and variant == "original": command.append("--check")
        subprocess.run(command, check=True)
    subprocess.run([sys.executable, "-B", str(g.HERE / "test_build_inputs.py")], check=True)
    subprocess.run([os.getenv("CARGO", "cargo"), "build", "--locked", "--release", "-p", "ogpu"], check=True)
    library = g.ROOT / "target/release"
    env = os.environ.copy(); env["LD_LIBRARY_PATH"] = str(library)+os.pathsep+env.get("LD_LIBRARY_PATH", "")
    for variant, directory in (("original", source), ("reordered", build / "changed source tree")):
        executable = build / ("dependency-"+variant)
        subprocess.run([*shlex.split(os.getenv("CC", "cc")), "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror",
                        "-I", str(directory), "-Iinclude", str(g.HERE / "affine.c"), "-L", str(library),
                        f"-Wl,-rpath,{library}", "-logpu", "-o", str(executable)], check=True)
        if not args.check:
            result = subprocess.run([str(executable)], capture_output=True, text=True, env=env)
            print(result.stdout, end=""); print(result.stderr, end="", file=sys.stderr)
            g.require(result.returncode == 0 and "Validation Error:" not in result.stdout+result.stderr
                      and "Affine PASS:" in result.stdout, "dependency consumer execution failed")
    print("Transitive dependency original/reordered " + ("build" if args.check else "execution") + " PASS")


if __name__ == "__main__": main()
