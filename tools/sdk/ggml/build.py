#!/usr/bin/env python3
"""Build the independent GGML consumer from a pinned upstream checkout and SDK."""
import argparse
import os
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--ggml-source",type=Path,required=True)
args = parser.parse_args()
pkg = os.getenv("PKG_CONFIG","pkg-config")
def variable(name):
    return subprocess.check_output([pkg,"--variable="+name,"ogpu-foundation"],text=True).strip()
subprocess.run(["cmake","-S",str(HERE),"-B",str(HERE / "build"),"-G","Ninja",
                "-DCMAKE_BUILD_TYPE=Release","-DOGPU_GGML_FOUNDATION=ON",
                "-DGGML_SOURCE="+str(args.ggml_source.resolve()),"-DOGPU_INCLUDE_DIR="+variable("includedir"),
                "-DOGPU_LIBRARY="+str(Path(variable("libdir")) / "libogpu.so")],check=True)
subprocess.run(["cmake","--build",str(HERE / "build"),"-j","8"],check=True)
print("Independent foundation GGML build PASS; run python3 run.py [--data-dir /path/to/mnist]")
