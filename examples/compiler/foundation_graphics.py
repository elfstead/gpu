#!/usr/bin/env python3
"""Link foundation stages offline; run unchanged public C against four variants."""
import argparse
import os
import shlex
import subprocess
import sys
sys.dont_write_bytecode = True
import generate as g
import link_graphics
from stage_workflow import VARIANTS, mutate


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="reproduce and build; no GPU execution")
    args = parser.parse_args()
    os.chdir(g.ROOT)
    g.require(not os.getenv("CARGO_TARGET_DIR"), "leave CARGO_TARGET_DIR unset")
    build = g.ROOT / "target/foundation-graphics"
    for variant in VARIANTS:
        folder = build / variant
        folder.mkdir(parents=True, exist_ok=True)
        sources = {}
        for stage in ("vertex", "fragment"):
            original = g.HERE / f"stage_{stage}.slang"
            sources[stage] = original if variant == "original" else folder / original.name
            if variant != "original": sources[stage].write_text(mutate(original.read_text(), variant, stage))
        content = link_graphics.header(sources["vertex"],sources["fragment"],folder,
                                      os.getenv("SLANGC","slangc"),"pattern",api="foundation")
        output = (g.HERE if variant == "original" else folder) / "foundation-pattern.generated.h"
        if args.check and variant == "original": g.require(output.read_text() == content,"stale foundation graphics header")
        else: output.write_text(content)
    subprocess.run([sys.executable,"-B",str(g.HERE / "test_foundation_graphics.py")],check=True)
    subprocess.run([os.getenv("CARGO","cargo"),"build","--locked","-p","ogpu"],check=True)
    library = g.ROOT / "target/debug"
    env = os.environ.copy(); env["LD_LIBRARY_PATH"] = str(library)+os.pathsep+env.get("LD_LIBRARY_PATH","")
    for variant in VARIANTS:
        headers = g.HERE if variant == "original" else build / variant
        executable = build / ("consumer-"+variant)
        subprocess.run([*shlex.split(os.getenv("CC","cc")),"-std=c11","-O2","-Wall","-Wextra","-Werror","-pedantic",
                        "-I",str(headers),"-Iinclude",str(g.HERE / "foundation_graphics.c"),"-L",str(library),
                        f"-Wl,-rpath,{library}","-logpu","-o",str(executable)],check=True)
        if not args.check:
            output = subprocess.run([str(executable)],capture_output=True,text=True,env=env)
            print(variant+":\n"+output.stdout,end=""); print(output.stderr,end="",file=sys.stderr)
            g.require(output.returncode == 0 and "Validation Error:" not in output.stdout+output.stderr
                      and output.stdout.count("PASS") == 6,"foundation graphics execution failed")
    print("Foundation graphics original/reordered/renamed/noperspective " + ("build" if args.check else "execution") + " PASS")


if __name__ == "__main__": main()
