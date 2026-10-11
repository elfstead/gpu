#!/usr/bin/env python3
"""Foundation migration acceptance against frozen oracles and the installed caller."""
import argparse
import os
import shlex
import sys
sys.dont_write_bytecode = True
# Load the sibling acceptance runner before its compiler adapter prepends another
# directory containing a run.py to sys.path.
import run as acceptance
generate_interfaces = acceptance.generate_interfaces


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="CPU/generator/build checks only; no GPU execution")
    args = parser.parse_args()
    root, here = acceptance.ROOT, acceptance.HERE
    os.chdir(root)
    acceptance.require(not os.getenv("CARGO_TARGET_DIR"), "leave CARGO_TARGET_DIR unset")
    acceptance.command(sys.executable,"-B",str(here / "check.py"))
    acceptance.command(sys.executable,"-B",str(here / "test_runner.py"))
    interfaces = generate_interfaces.build_interfaces(args.check,api="foundation")
    acceptance.command(sys.executable,"-B",str(here / "test_interfaces.py"),"--foundation")
    installed = generate_interfaces.build_interfaces(True)
    acceptance.command(os.getenv("CARGO","cargo"),"build","--locked","--release","-p","ogpu")
    build, library = root / "target/learned-image", root / "target/release"
    executables = {}
    for variant, headers, source in [("foundation-"+name,headers,"foundation.c") for name,headers in interfaces.items()] + [
            ("installed-control",installed["original"],"app.c")]:
        executable = build / ("app-"+variant)
        acceptance.command(*shlex.split(os.getenv("CC","cc")),"-std=c11","-O2","-Wall","-Wextra","-Werror","-pedantic",
                           "-I",str(headers),"-Iinclude",str(here / source),"-L",str(library),f"-Wl,-rpath,{library}","-logpu","-o",str(executable))
        executables[variant] = executable
    if args.check:
        print("Learned-image foundation original/mutated and installed-control build PASS; no GPU execution")
        return
    outputs = {name: acceptance.execute_variant(build,library,executable,name) for name,executable in executables.items()}
    reference = "foundation-original"
    for variant in ("foundation-mutated","installed-control"):
        acceptance.require(outputs[reference] == outputs[variant],"migration output sets differ")
        for path in outputs[reference]:
            acceptance.require(acceptance.same_file(build / "gpu/small" / reference / path,build / "gpu/small" / variant / path),
                               f"migration changed output bytes: {variant}/{path}")
    print("Learned-image foundation migration PASS: frozen numerical gates; every output byte matches reordered interfaces and installed control")


if __name__ == "__main__": main()
