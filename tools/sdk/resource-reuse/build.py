#!/usr/bin/env python3
"""Build the copied consumer using only C11 and the installed ogpu.pc."""
import os
from pathlib import Path
import shlex
import subprocess

here = Path(__file__).resolve().parent
pkg = os.getenv("PKG_CONFIG", "pkg-config")
flags = shlex.split(subprocess.check_output([pkg, "--cflags", "--libs", "ogpu"], text=True))
libdir = subprocess.check_output([pkg, "--variable=libdir", "ogpu"], text=True).strip()
subprocess.run([*shlex.split(os.getenv("CC", "cc")), "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror",
                "-MMD", "-MF", str(here / "reuse.d"), "-I", str(here), str(here / "main.c"),
                *flags, f"-Wl,-rpath,{libdir}", "-o", str(here / "reuse")], check=True)
print(f"Built {here / 'reuse'}; run python3 run.py")
