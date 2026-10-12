"""Installed-tool profile selection. No silent redirection of legacy shaders."""
from pathlib import Path


def select(argv, prefix):
    args=list(argv)
    explicit=any(word=='--api' or word.startswith('--api=') for word in args)
    if not explicit:args+=['--api','foundation']
    # Argparse in the underlying tools validates malformed/duplicate options;
    # do not run a compiler for a legacy profile absent from this SDK.
    selected=None
    for i,word in enumerate(args):
        if word=='--api' and i+1<len(args):selected=args[i+1]
        elif word.startswith('--api='):selected=word.split('=',1)[1]
    if selected=='installed' and not (Path(prefix)/'include/ogpu.h').is_file():
        raise ValueError('ABI20 is not packaged; install with --legacy and select --api installed explicitly')
    return args
