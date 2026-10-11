#!/usr/bin/env python3
"""Build the copied preview using only C11 and ogpu-foundation.pc."""
import os
from pathlib import Path
import shlex
import subprocess

here = Path(__file__).resolve().parent
pkg = os.getenv("PKG_CONFIG","pkg-config")
flags = shlex.split(subprocess.check_output([pkg,"--cflags","--libs","ogpu-foundation"],text=True))
libdir = subprocess.check_output([pkg,"--variable=libdir","ogpu-foundation"],text=True).strip()
subprocess.run([*shlex.split(os.getenv("CC","cc")),"-std=c11","-O2","-Wall","-Wextra","-Werror","-pedantic",
                "-MMD","-MF",str(here / "consumer.d"),"-I",str(here),str(here / "main.c"),
                *flags,f"-Wl,-rpath,{libdir}","-lm","-o",str(here / "consumer")],check=True)
print(f"Built {here / 'consumer'} from the revision-pinned foundation preview")
