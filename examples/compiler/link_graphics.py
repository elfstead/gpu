#!/usr/bin/env python3
"""Compile/check a bounded graphics pair and embed its unmodified stage artifacts."""
import argparse
import os
from pathlib import Path
import re
import generate as g


def header(vertex, fragment, build, compiler, name, native_heaps=False, include_dirs=(), build_info=None):
    g.require(re.fullmatch(r"[a-z][a-z0-9_]*", name), "unsupported pair name")
    artifacts, descriptions = {}, {}
    for stage, source in (("vertex", vertex), ("fragment", fragment)):
        info = {} if build_info is not None else None
        reflection, assembly, binary = g.compile_source(source, build / stage, compiler, stage, native_heaps, include_dirs, info)
        if build_info is not None: build_info.append(info)
        descriptions[stage] = g.inspect(reflection, assembly, name+"_"+stage)
        artifacts[stage] = g.header(reflection, assembly, binary, source.read_bytes(), name+"_"+stage)
    inputs = g.stages.link(descriptions["vertex"][0].io, descriptions["fragment"][0].io)
    guard = "OGPU_GENERATED_"+name.upper()+"_PAIR_H"
    lines = ["/* Offline checked graphics pair; no runtime linking/reflection or code rewriting. */",
             f"#ifndef {guard}", f"#define {guard}"]
    for item in inputs:
        lines.append(f"/* Location {item['location']}: {item['scalar']} x {item['lanes']}, fragment {item['interpolation']}. */")
    lines += [artifacts["vertex"], artifacts["fragment"],
              f"enum {{ {name}_push_size = {max(descriptions[s][1] for s in descriptions)} }};", "#endif", ""]
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--vertex-source", required=True, type=Path)
    parser.add_argument("--fragment-source", required=True, type=Path)
    parser.add_argument("--build-dir", required=True, type=Path)
    parser.add_argument("--name", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--native-heaps", action="store_true")
    parser.add_argument("--check", action="store_true")
    g.build_inputs.add_options(parser)
    args = parser.parse_args()
    g.build_inputs.validate_options(args)
    info = [] if args.manifest else None
    content = header(args.vertex_source, args.fragment_source, args.build_dir,
                     os.getenv("SLANGC", "slangc"), args.name, args.native_heaps, args.include_dir, info)
    g.build_inputs.publish(args.output, content, args, info, "stale generated graphics pair")
    print("Graphics pair generation/link PASS")


if __name__ == "__main__": main()
