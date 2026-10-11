#!/usr/bin/env python3
"""Build the independent foundation/libplacebo consumer, without an OGPU checkout."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
PIN = "3330a515d62139259c26239014f286e233bd3a5c"


def digest(path):
    with path.open("rb") as file: return hashlib.file_digest(file,"sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--libplacebo-source",type=Path,required=True)
    parser.add_argument("--vulkan-registry",type=Path,required=True,help="vk.xml matching the build environment's Vulkan headers")
    args = parser.parse_args()
    source = args.libplacebo_source.resolve()
    def output(command): return subprocess.check_output(command,text=True).strip()
    def run(command): subprocess.run(command,check=True)
    if output(["git","-C",str(source),"rev-parse","HEAD"]) != PIN:
        raise ValueError("wrong libplacebo source revision")
    run(["git","-C",str(source),"diff","--exit-code","HEAD","--"])
    registry = args.vulkan_registry.resolve(strict=True)
    pkg = os.getenv("PKG_CONFIG","pkg-config")
    def flags(option,*packages): return shlex.split(output([pkg,option,*packages]))
    def variable(name): return output([pkg,"--variable="+name,"ogpu-foundation"])
    if variable("ogpu_foundation") != "1": raise ValueError("foundation revision 1 required")
    include,library = Path(variable("includedir")).resolve(),Path(variable("libdir")).resolve()
    # Fresh build every time; failed builds never replace the last complete receipt.
    build = Path(tempfile.mkdtemp(prefix="build-",dir=HERE))
    upstream = build / "upstream"
    print(f"Independent libplacebo build: {build}",flush=True)
    run(["meson","setup",str(upstream),str(source),"--buildtype=release","--default-library=shared","--wrap-mode=nodownload",
         "-Dvulkan=enabled","-Dvk-proc-addr=enabled","-Dshaderc=enabled","-Dglslang=disabled",
         "-Dvulkan-registry="+str(registry),"-Dopengl=disabled","-Dd3d11=disabled","-Dlcms=disabled",
         "-Ddovi=disabled","-Dlibdovi=disabled","-Dunwind=disabled","-Dxxhash=disabled",
         "-Ddemos=false","-Dtests=false","-Dbench=false"])
    run(["meson","compile","-C",str(upstream),"-j","8"])
    cc,cxx = shlex.split(os.getenv("CC","cc")),shlex.split(os.getenv("CXX","c++"))
    common = ["-O2","-Wall","-Wextra","-Werror"]
    includes = ["-I"+str(p) for p in (source / "src",upstream / "src",source / "src/include",upstream / "src/include",include)]
    for unit in ("backend","consumer","backend-tests","stream","perf","reference","hdr","compare-hdr"):
        run([*cc,"-std=gnu11",*common,"-Wno-unused-parameter",*includes,*flags("--cflags","vulkan"),
             "-MMD","-MF",str(build / (unit+".d")),"-c",str(HERE / (unit+".c")),"-o",str(build / (unit+".o"))])
    for unit in ("compiler","metadata","compare-reference"):
        run([*cxx,"-std=c++17",*common,*flags("--cflags","shaderc","SPIRV-Tools"),
             "-MMD","-MF",str(build / (unit+".d")),"-c",str(HERE / (unit+".cpp")),"-o",str(build / (unit+".o"))])
    upstream_lib = upstream / "src"
    links = ["-L"+str(library),"-Wl,-rpath,"+str(library),"-logpu",
             "-L"+str(upstream_lib),"-Wl,-rpath,"+str(upstream_lib),"-lplacebo",
             *flags("--libs","shaderc","SPIRV-Tools"),"-lm"]
    for unit in ("consumer","backend-tests","stream","perf","hdr"):
        wrappers = ["-Wl,--wrap="+name for name in ("ogpu_next_timeline_poll","ogpu_next_queue_submit",
                    "ogpu_next_commands_end","ogpu_next_timeline_wait")] if unit == "backend-tests" else []
        run([*cxx,*[str(build / (n+".o")) for n in (unit,"backend","compiler","metadata")],*wrappers,*links,"-o",str(build / unit)])
    run([*cc,str(build / "reference.o"),"-L"+str(upstream_lib),"-Wl,-rpath,"+str(upstream_lib),"-lplacebo","-lm","-o",str(build / "reference")])
    run([*cc,str(build / "compare-hdr.o"),"-lm","-o",str(build / "compare-hdr")])
    run([*cxx,str(build / "compare-reference.o"),"-o",str(build / "compare-reference")])
    record = dict(schema=1,complete=True,build=str(build),upstream_revision=PIN,
                  sdk=dict(revision=variable("ogpu_revision"),runtime_sha256=digest(library / "libogpu.so"),
                           header_sha256=digest(include / "ogpu_next.h")),
                  upstream_library_sha256=digest(upstream_lib / "libplacebo.so"),registry_sha256=digest(registry))
    record["sources_sha256"] = {p.name:digest(p) for p in sorted(HERE.iterdir()) if p.suffix in (".c",".cpp",".h",".hpp",".py")}
    record["executables_sha256"] = {n:digest(build / n) for n in ("consumer","backend-tests","stream","perf","hdr","reference","compare-hdr","compare-reference")}
    (build / "build.json").write_text(json.dumps(record,indent=2)+"\n")
    (HERE / "build.json").write_text(json.dumps(record,indent=2)+"\n")
    print("Independent foundation libplacebo build PASS; run python3 run.py")


if __name__ == "__main__": main()
