#!/usr/bin/env python3
"""Build fresh-recording and replay callers using only the installed preview."""
import os
from pathlib import Path
import shlex
import subprocess

here=Path(__file__).resolve().parent
pkg=os.getenv('PKG_CONFIG','pkg-config')
flags=shlex.split(subprocess.check_output([pkg,'--cflags','--libs','ogpu-foundation'],text=True))
libdir=subprocess.check_output([pkg,'--variable=libdir','ogpu-foundation'],text=True).strip()
for source,output in (('main.c','consumer'),('reuse_main.c','reuse')):
    subprocess.run([*shlex.split(os.getenv('CC','cc')),'-std=c11','-O2','-Wall','-Wextra','-Werror',
        '-MMD','-MF',str(here/(output+'.d')),'-I',str(here),str(here/source),*flags,
        '-Wl,-rpath,'+libdir,'-ldl','-o',str(here/output)],check=True)
    print(f'Built {output} from the installed foundation preview',flush=True)
