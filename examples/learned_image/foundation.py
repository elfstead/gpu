#!/usr/bin/env python3
"""Foundation migration acceptance against frozen oracles and the installed caller."""
import argparse
import json
import os
import shlex
import sys
import tempfile
from pathlib import Path
sys.dont_write_bytecode = True
# Load the sibling acceptance runner before its compiler adapter prepends another
# directory containing a run.py to sys.path.
import run as acceptance
import benchmark
generate_interfaces = acceptance.generate_interfaces


def validate_reuse(build, executables):
    """Use the existing A/B/A oracles, not a new GPU golden or looser gate."""
    destination = Path(tempfile.mkdtemp(prefix="foundation-reuse-",dir=build))
    print(f"Foundation reuse artifacts: {destination}",flush=True)
    reference = build / "reference"
    manifest = json.loads((reference / "manifest.json").read_text())
    extent = (65,47,131,95)
    cases = [c for c in manifest["cases"] if tuple(c["input_size"]+c["output_size"]) == extent
             and c["seed"] in (2001,2002)][:2]
    acceptance.require(len(cases) == 2 and [c["seed"] for c in cases] == [2001,2002],"missing reuse fixtures")
    acceptance.verified(acceptance.HERE / "model.json",manifest["model_sha256"])
    acceptance.verified(reference / "weights.f32",manifest["weights_sha256"])
    for case in cases:
        for name,digest in case["sha256"].items(): acceptance.verified(reference / case["name"] / name,digest)
    baseline = None
    for variant,executable in executables.items():
        for mode in benchmark.MODES:
            output = destination / f"{variant}-{mode}"
            benchmark.invoke(executable,"--validate",mode,extent,reference,cases,output,os.environ.copy())
            benchmark.check_validation(output,reference,[cases[0],cases[1],cases[0]])
            if baseline is None: baseline = output
            else:
                for name in benchmark.output_files():
                    acceptance.require(acceptance.same_file(output / name,baseline / name),f"reuse migration mismatch: {output / name}")
    print("Foundation resident/end-to-end A/B/A replay: frozen oracles and all three callers byte-identical PASS")


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
    validate_reuse(build,executables)


if __name__ == "__main__": main()
