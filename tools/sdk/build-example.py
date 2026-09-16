#!/usr/bin/env python3
"""Standalone C11 example build. Needs installed ogpu.pc, not Cargo or a checkout."""
import os
from pathlib import Path
import shlex
import subprocess

here = Path(__file__).resolve().parent
pkg_config = os.getenv("PKG_CONFIG", "pkg-config")
flags = shlex.split(subprocess.check_output([pkg_config, "--cflags", "--libs", "ogpu"], text=True))
libdir = subprocess.check_output([pkg_config, "--variable=libdir", "ogpu"], text=True).strip()
subprocess.run([*shlex.split(os.getenv("CC", "cc")), "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror",
                "-I", str(here), str(here / "main.c"), *flags, f"-Wl,-rpath,{libdir}",
                "-o", str(here / "transform")], check=True)
print(f"Built {here / 'transform'}; run it to check device compatibility and execute")
