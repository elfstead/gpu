#!/usr/bin/env python3
"""Relocate an opt-in preview and check independent public-only consumers."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile


def require(value,message):
    if not value: raise ValueError(message)


def run(command,**kwargs):
    p = subprocess.run(command,text=True,capture_output=True,timeout=300,**kwargs)
    print(p.stdout,end="",flush=True); print(p.stderr,end="",file=sys.stderr,flush=True)
    require(p.returncode == 0,f"command failed: {command}")
    require("Validation Error:" not in p.stdout+p.stderr,"Vulkan validation error")
    return p.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prefix",type=Path,required=True)
    parser.add_argument("--no-gpu",action="store_true")
    parser.add_argument("--shader-check",action="store_true")
    args = parser.parse_args()
    installed = args.prefix.resolve()
    manifest = json.loads((installed / "share/ogpu/manifest.json").read_text())
    require(manifest["foundation"]["revision"] == 1,"not a foundation revision-1 preview")
    for relative,digest in manifest["files_sha256"].items():
        require(hashlib.sha256((installed / relative).read_bytes()).hexdigest() == digest,f"installed hash mismatch: {relative}")
    temporary = Path(tempfile.mkdtemp(prefix="ogpu-foundation-external-"))
    prefix = temporary / "relocated sdk"; shutil.copytree(installed,prefix)
    print(f"Foundation external artifacts: {temporary}",flush=True)
    env = os.environ.copy()
    env["PKG_CONFIG_PATH"] = env["PKG_CONFIG_LIBDIR"] = str(prefix / "lib/pkgconfig")
    for key in ("PKG_CONFIG_SYSROOT_DIR","LD_LIBRARY_PATH","LD_PRELOAD","LIBRARY_PATH","CPATH","C_INCLUDE_PATH","CPLUS_INCLUDE_PATH","PYTHONPATH"):
        env.pop(key,None)
    env["CARGO"] = "/no-consumer-cargo-required"; env["SLANGC"] = "/no-consumer-slang-required"
    pkg = os.getenv("PKG_CONFIG","pkg-config")
    require(run([pkg,"--variable=ogpu_revision","ogpu-foundation"],env=env).strip() == manifest["revision"],"wrong revision")
    require(run([pkg,"--variable=ogpu_foundation","ogpu-foundation"],env=env).strip() == "1","wrong foundation version")
    for label in ("compute","graphics","learned-image"):
        app = temporary / ("independent "+label)
        shutil.copytree(prefix / ("share/ogpu/examples/foundation-"+label),app)
        run([sys.executable,"-B","build.py"],cwd=app,env=env)
        dependencies = [Path(p).resolve() for p in shlex.split((app / "consumer.d").read_text().replace("\\\n",""))[1:]]
        require(prefix / "include/ogpu_next.h" in dependencies,"preview header missing")
        require(all(p.is_relative_to(prefix) or p.is_relative_to(app) for p in dependencies),"source/header escaped installed graph")
        require(not any("vulkan" in p.name.lower() or p.name == "ogpu.h" for p in dependencies),"consumer imported legacy/native headers")
        linked = run(["ldd","./consumer"],cwd=app,env=env)
        selected = next((line.split("=>",1)[1].rsplit(" (",1)[0].strip() for line in linked.splitlines() if line.strip().startswith("libogpu.so =>")),None)
        require(selected is not None and Path(selected).resolve() == prefix / "lib/libogpu.so","wrong runtime resolved")
        missing = dict(env,OGPU_VULKAN_LIBRARY="/deliberately-missing-ogpu-loader.so")
        command = [sys.executable,"-B","run.py"] if label == "learned-image" else ["./consumer"]
        failed = subprocess.run(command,cwd=app,env=missing,capture_output=True,text=True,timeout=60)
        require(failed.returncode != 0,"missing loader succeeded")
        if label == "learned-image":
            reports = list(app.glob("run-*/report.json")); require(len(reports) == 1,"missing failure receipt")
            receipt = json.loads(reports[0].read_text())
            require(not receipt["complete"] and receipt.get("error") and not receipt["runs"],"false success after missing loader")
        if not args.no_gpu:
            run(command,cwd=app,env=env)
            if label == "learned-image":
                complete = [json.loads(p.read_text()) for p in app.glob("run-*/report.json") if json.loads(p.read_text())["complete"]]
                require(len(complete) == 1 and len(complete[0]["runs"]) == 2,"missing completed handoff receipt")
                require(complete[0]["sdk"]["revision"] == manifest["revision"] and
                        complete[0]["sdk"]["runtime_sha256"] == manifest["files_sha256"]["lib/libogpu.so"],"handoff identity mismatch")
        if args.shader_check and label != "learned-image":
            shader_env = dict(env,SLANGC=os.getenv("SLANGC","slangc"))
            if label == "compute":
                generator = [str(prefix / "bin/ogpu-shader"),"--api","foundation","--source","transform.slang","--output",
                             "foundation.generated.h","--build-dir","shader-build","--name","transform","--stage","compute"]
            else:
                generator = [str(prefix / "bin/ogpu-graphics"),"--api","foundation","--vertex-source","stage_vertex.slang",
                             "--fragment-source","stage_fragment.slang","--output","foundation-pattern.generated.h",
                             "--build-dir","shader-build","--name","pattern"]
            run([*generator,"--check"],cwd=app,env=shader_env)
            if label == "compute":
                source = app / "transform.slang"; original = source.read_text()
                changed = original.replace("    uint* data;\n    uint count;","    uint count;\n    uint* data;").replace("numthreads(64, 1, 1)","numthreads(32, 1, 1)")
                require(changed != original,"mutation did not change shader"); source.write_text(changed)
                stale = subprocess.run([*generator,"--check"],cwd=app,env=shader_env,capture_output=True,text=True,timeout=120)
                require(stale.returncode != 0 and "stale generated interface" in stale.stderr,"stale header accepted")
                run(generator,cwd=app,env=shader_env)
                run([sys.executable,"-B","build.py"],cwd=app,env=env)
                if not args.no_gpu: run(["./consumer"],cwd=app,env=env)
    print("Relocated foundation preview source graph, runtime identity and failure handling PASS")


if __name__ == "__main__": main()
