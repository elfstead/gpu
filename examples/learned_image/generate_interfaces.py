#!/usr/bin/env python3
"""Application-owned list of sources; shared compiler adapter owns mechanics."""
import argparse
import os
from pathlib import Path
import re
import sys
sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE.parent / "compiler"))
import generate

SOURCES = (("hidden", "compute"), ("denoise", "compute"), ("process", "compute"),
           ("poison", "compute"), ("fullscreen", "vertex"), ("display", "fragment"))


def mutated(source, stage):
    if stage == "vertex":
        return source  # This executable has no root/workgroup interface to mutate.
    match = re.search(r"struct Arguments \{\n(.*?)\n\};", source, re.S)
    generate.require(match is not None, "missing mutation root")
    fields = match.group(1).splitlines()
    generate.require(all(re.fullmatch(r"    (float\*|uint\*|uint) \w+;", f) for f in fields),
                     "mutation expects flat one-field-per-line roots")
    changed = source[:match.start(1)] + "\n".join(reversed(fields)) + source[match.end(1):]
    if stage == "compute":
        generate.require(source.count("numthreads(64, 1, 1)") == 1, "missing mutation local size")
        changed = changed.replace("numthreads(64, 1, 1)", "numthreads(32, 1, 1)")
    generate.require(changed != source, "mutation did not change interface")
    return changed


def build_interfaces(check=False):
    build = ROOT / "target/learned-image/compiler"
    outputs = {}
    compiler = os.getenv("SLANGC", "slangc")
    originals = {}
    for variant in ("original", "mutated"):
        headers = HERE / "generated" if variant == "original" else build / variant / "generated"
        headers.mkdir(parents=True, exist_ok=True)
        for name, stage in SOURCES:
            source = HERE / f"{name}.slang"
            directory = build / variant / name
            directory.mkdir(parents=True, exist_ok=True)
            if variant == "mutated":
                altered = directory / f"{name}.slang"
                altered.write_text(mutated(source.read_text(), stage))
                source = altered
            reflection, assembly, binary = generate.compile_source(source, directory, compiler, stage)
            metadata = generate.inspect(reflection, assembly)
            if variant == "original":
                originals[name] = metadata
            elif stage != "vertex":
                generate.require(metadata[0] != originals[name][0], "root order did not change")
                generate.require(metadata[3] == ([32, 1, 1] if stage == "compute" else []),
                                 "mutated local dimensions mismatch")
            generated = generate.header(reflection, assembly, binary, source.read_bytes(), name)
            output = headers / f"{name}.generated.h"
            if check and variant == "original":
                generate.require(output.read_text() == generated, f"stale generated interface: {name}")
            else:
                output.write_text(generated)
        umbrella = ("/* Generated executable interfaces; application policy is not generated. */\n"
                    "#ifndef OGPU_LEARNED_IMAGE_APPLICATION_H\n#define OGPU_LEARNED_IMAGE_APPLICATION_H\n"
                    + "".join(f'#include "{name}.generated.h"\n' for name, _ in SOURCES) + "#endif\n")
        output = headers / "application.generated.h"
        if check and variant == "original":
            generate.require(output.read_text() == umbrella, "stale generated umbrella")
        else:
            output.write_text(umbrella)
        outputs[variant] = headers
    print("Six compiler-derived interfaces plus root-order/local-size mutation PASS", flush=True)
    return outputs


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    build_interfaces(parser.parse_args().check)
