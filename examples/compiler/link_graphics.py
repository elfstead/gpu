#!/usr/bin/env python3
"""Compile/check a bounded graphics pair and embed its unmodified stage artifacts."""
import argparse
import os
from pathlib import Path
import re
import generate as g


def header(vertex, fragment, build, compiler, name, native_heaps=False, include_dirs=(), build_info=None, api="installed"):
    g.require(re.fullmatch(r"[a-z][a-z0-9_]*", name), "unsupported pair name")
    artifacts, descriptions = {}, {}
    for stage, source in (("vertex", vertex), ("fragment", fragment)):
        info = {} if build_info is not None else None
        reflection, assembly, binary = g.compile_source(source, build / stage, compiler, stage, native_heaps, include_dirs, info)
        if build_info is not None: build_info.append(info)
        descriptions[stage] = g.inspect(reflection, assembly, name+"_"+stage)
        artifacts[stage] = g.header(reflection, assembly, binary, source.read_bytes(), name+"_"+stage, api)
    inputs = g.stages.link(descriptions["vertex"][0].io, descriptions["fragment"][0].io)
    guard = "OGPU_GENERATED_"+name.upper()+("_FOUNDATION_PAIR_H" if api == "foundation" else "_PAIR_H")
    lines = ["/* Offline checked graphics pair; no runtime linking/reflection or code rewriting. */",
             f"#ifndef {guard}", f"#define {guard}"]
    for item in inputs:
        lines.append(f"/* Location {item['location']}: {item['scalar']} x {item['lanes']}, fragment {item['interpolation']}. */")
    lines += [artifacts["vertex"], artifacts["fragment"],
              f"enum {{ {name}_push_size = {max(descriptions[s][1] for s in descriptions)} }};"]
    if api == "foundation":
        lines += foundation_pair(name)
    lines += ["#endif", ""]
    return "\n".join(lines)


def foundation_pair(name, vertex=None, fragment=None):
    vertex, fragment = vertex or name+"_vertex", fragment or name+"_fragment"
    return ["/* One shared native inline bank, not independently snapshotted stage banks.",
            " * Stage structs are views of these SAME bytes. The caller must supply",
            " * simultaneously valid values; this linker never relocates or packs roots. */",
            f"#define {name}_required_features ({vertex}_required_features | {fragment}_required_features)",
            f"static const ogpu_next_argument_interface {name}_interface = {{",
            "    .header = {OGPU_NEXT_ARGUMENT_INTERFACE, OGPU_NEXT_VERSION, sizeof(ogpu_next_argument_interface), 0, NULL},",
            f"    .byte_size = {name}_push_size, .root_count = 0, .roots = NULL}};",
            f"static const ogpu_next_shader_requirements {name}_requirements = {{",
            "    .header = {OGPU_NEXT_SHADER_REQUIREMENTS, OGPU_NEXT_VERSION, sizeof(ogpu_next_shader_requirements), 0, NULL},",
            f"    .features = {name}_required_features, .local_size = {{0, 0, 0}}, .shared_memory = 0}};",
            f"static inline void {name}_shaders(ogpu_next_shader out[2]) {{",
            f"    out[0] = {vertex}_shader(); out[1] = {fragment}_shader();",
            f"    out[0].interface_metadata = out[1].interface_metadata = &{name}_interface.header;", "}",
            f"static inline int {name}_compatible(const ogpu_next_feature_info *c, const ogpu_next_execution_limits *l) {{",
            f"    return {vertex}_compatible(c, l) && {fragment}_compatible(c, l);", "}"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--vertex-source", required=True, type=Path)
    parser.add_argument("--fragment-source", required=True, type=Path)
    parser.add_argument("--build-dir", required=True, type=Path)
    parser.add_argument("--name", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--native-heaps", action="store_true")
    parser.add_argument("--api", choices=("installed", "foundation"), default="installed")
    parser.add_argument("--check", action="store_true")
    g.build_inputs.add_options(parser)
    args = parser.parse_args()
    g.build_inputs.validate_options(args)
    info = [] if args.manifest else None
    content = header(args.vertex_source, args.fragment_source, args.build_dir,
                     os.getenv("SLANGC", "slangc"), args.name, args.native_heaps, args.include_dir, info, args.api)
    g.build_inputs.publish(args.output, content, args, info, "stale generated graphics pair")
    print("Graphics pair generation/link PASS")


if __name__ == "__main__": main()
