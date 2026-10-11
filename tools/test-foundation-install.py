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


def run(command,quiet=False,**kwargs):
    p = subprocess.run(command,text=True,capture_output=True,timeout=300,**kwargs)
    if not quiet: print(p.stdout,end="",flush=True)
    print(p.stderr,end="",file=sys.stderr,flush=True)
    require(p.returncode == 0,f"command failed: {command}")
    require("Validation Error:" not in p.stdout+p.stderr,"Vulkan validation error")
    return p.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prefix",type=Path,required=True)
    parser.add_argument("--no-gpu",action="store_true")
    parser.add_argument("--shader-check",action="store_true")
    parser.add_argument("--ggml-source",type=Path,help="also build/check the independent pinned GGML consumer")
    parser.add_argument("--ggml-data",type=Path,help="optional extracted MNIST data for full GGML inference")
    parser.add_argument("--libplacebo-source",type=Path,help="also build/check the independent pinned libplacebo consumer")
    parser.add_argument("--vulkan-registry",type=Path,help="matching vk.xml, required with --libplacebo-source")
    args = parser.parse_args()
    require(not args.ggml_data or args.ggml_source,"--ggml-data needs --ggml-source")
    require(bool(args.libplacebo_source) == bool(args.vulkan_registry),"--libplacebo-source and --vulkan-registry belong together")
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
    for label in ("compute","graphics","learned-image","indexed-scene"):
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
        scripted = label in ("learned-image","indexed-scene")
        command = [sys.executable,"-B","run.py"] if scripted else ["./consumer"]
        failed = subprocess.run(command,cwd=app,env=missing,capture_output=True,text=True,timeout=60)
        require(failed.returncode != 0,"missing loader succeeded")
        if scripted:
            reports = list(app.glob("run-*/report.json")); require(len(reports) == 1,"missing failure receipt")
            receipt = json.loads(reports[0].read_text())
            require(not receipt["complete"] and receipt.get("error") and not receipt["runs"],"false success after missing loader")
        if not args.no_gpu:
            run(command,cwd=app,env=env)
            if scripted:
                complete = [json.loads(p.read_text()) for p in app.glob("run-*/report.json") if json.loads(p.read_text())["complete"]]
                require(len(complete) == 1 and len(complete[0]["runs"]) == 2,"missing completed handoff receipt")
                require(complete[0]["sdk"]["revision"] == manifest["revision"] and
                        complete[0]["sdk"]["runtime_sha256"] == manifest["files_sha256"]["lib/libogpu.so"],"handoff identity mismatch")
        if args.shader_check and label in ("compute","graphics"):
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
    if args.ggml_source:
        app = temporary / "independent ggml"
        shutil.copytree(prefix / "share/ogpu/examples/foundation-ggml",app)
        source = temporary / "upstream ggml"
        # A separate local checkout removes even the upstream source dependency on
        # the OGPU checkout's target directory; no network or upstream modification.
        run(["git","clone","--local","--no-hardlinks","--no-checkout",str(args.ggml_source.resolve()),str(source)])
        run(["git","-C",str(source),"checkout","--detach","7840aaba1989c6deeefede1d77d5aaf8f52b947e"])
        run([sys.executable,"-B","build.py","--ggml-source",str(source)],cwd=app,env=env)
        # Ninja's compiler dependency graph includes the exact selected header.
        dependencies = run(["ninja","-C","build","-t","deps"],quiet=True,cwd=app,env=env)
        (temporary / "ggml-dependencies.txt").write_text(dependencies)
        paths = [Path(line.strip()).resolve() for line in dependencies.splitlines() if line.startswith("    /")]
        require(prefix / "include/ogpu_next.h" in paths,"GGML did not use preview header")
        require(not any(p.name == "ogpu.h" or "vulkan" in p.name.lower() for p in paths),"GGML imported legacy/native headers")
        checkout = Path(__file__).resolve().parents[1]
        require(not any(p.is_relative_to(checkout) for p in paths),"GGML compilation depended on OGPU checkout")
        linked = run(["ldd","build/ogpu-mnist"],cwd=app,env=env)
        selected = next((line.split("=>",1)[1].rsplit(" (",1)[0].strip() for line in linked.splitlines() if line.strip().startswith("libogpu.so =>")),None)
        require(selected is not None and Path(selected).resolve() == prefix / "lib/libogpu.so","GGML resolved wrong runtime")
        missing = dict(env,OGPU_VULKAN_LIBRARY="/deliberately-missing-ogpu-loader.so")
        failed = subprocess.run([sys.executable,"-B","run.py"],cwd=app,env=missing,capture_output=True,text=True,timeout=60)
        reports = list(app.glob("run-*/report.json"))
        require(failed.returncode != 0 and len(reports) == 1,"GGML missing-loader rejection failed")
        receipt = json.loads(reports[0].read_text())
        require(not receipt["complete"] and receipt.get("error") and not receipt["runs"],"GGML failure reported success")
        if not args.no_gpu:
            command = [sys.executable,"-B","run.py"]
            if args.ggml_data: command += ["--data-dir",str(args.ggml_data.resolve())]
            run(command,cwd=app,env=env)
            complete = [json.loads(p.read_text()) for p in app.glob("run-*/report.json") if json.loads(p.read_text())["complete"]]
            require(len(complete) == 1 and len(complete[0]["runs"]) == (4 if args.ggml_data else 2),"incomplete GGML acceptance")
            require(complete[0]["sdk"]["revision"] == manifest["revision"] and
                    complete[0]["sdk"]["runtime_sha256"] == manifest["files_sha256"]["lib/libogpu.so"],"GGML SDK identity mismatch")
        print("Independent foundation GGML source graph, runtime identity and failure report PASS")
    if args.libplacebo_source:
        app = temporary / "independent libplacebo"
        shutil.copytree(prefix / "share/ogpu/examples/foundation-libplacebo",app)
        source = temporary / "upstream libplacebo"
        run(["git","clone","--local","--no-hardlinks","--no-checkout",str(args.libplacebo_source.resolve()),str(source)])
        run(["git","-C",str(source),"checkout","--detach","3330a515d62139259c26239014f286e233bd3a5c"])
        # The upstream/native reference needs external shaderc, SPIRV-Tools,
        # Vulkan and Python/Jinja packages, unlike the small public-only examples.
        dependency_env = dict(env)
        dependency_env["PKG_CONFIG_PATH"] = str(prefix / "lib/pkgconfig")+os.pathsep+os.getenv("PKG_CONFIG_PATH","")
        dependency_env.pop("PKG_CONFIG_LIBDIR",None)
        if "PYTHONPATH" in os.environ: dependency_env["PYTHONPATH"] = os.environ["PYTHONPATH"]
        run([sys.executable,"-B","build.py","--libplacebo-source",str(source),"--vulkan-registry",str(args.vulkan_registry.resolve())],cwd=app,env=dependency_env)
        record = json.loads((app / "build.json").read_text()); build = Path(record["build"])
        checkout = Path(__file__).resolve().parents[1]
        for dependency_file in build.glob("*.d"):
            dependencies = [Path(p).resolve() for p in shlex.split(dependency_file.read_text().replace("\\\n",""))[1:]]
            require(not any(p.is_relative_to(checkout) for p in dependencies),"libplacebo source escaped installed graph")
            require(not any(p.name == "ogpu.h" for p in dependencies),"libplacebo imported legacy OGPU API")
            if dependency_file.stem in ("backend","backend-tests","perf"):
                require(prefix / "include/ogpu_next.h" in dependencies,"libplacebo preview header missing")
        missing = dict(dependency_env,OGPU_VULKAN_LIBRARY="/deliberately-missing-ogpu-loader.so")
        # No-GPU machines need no ICD for loader-open rejection. This existing
        # non-ICD file passes the runner's path check but must never be parsed.
        missing.setdefault("VK_DRIVER_FILES",str(args.vulkan_registry.resolve()))
        failed = subprocess.run([sys.executable,"-B","run.py"],cwd=app,env=missing,capture_output=True,text=True,timeout=60)
        require(failed.returncode != 0,"libplacebo missing loader succeeded")
        reports = list(app.glob("run-*/report.json")); require(len(reports) == 1,"missing libplacebo failure receipt")
        receipt = json.loads(reports[0].read_text())
        require(not receipt["complete"] and receipt.get("error") and not receipt["runs"],"false libplacebo success on missing loader")
        # SDK identity must be checked even in no-GPU mode: changing source bytes
        # after build cannot silently reuse a successful old executable receipt.
        original = (app / "backend.c").read_bytes()
        (app / "backend.c").write_bytes(original+b"\n/* changed after build */\n")
        stale = subprocess.run([sys.executable,"-B","run.py"],cwd=app,env=dependency_env,capture_output=True,text=True,timeout=60)
        (app / "backend.c").write_bytes(original)
        require(stale.returncode != 0 and "consumer source changed since build" in stale.stderr,"stale libplacebo build accepted")
        if not args.no_gpu:
            run([sys.executable,"-B","run.py"],cwd=app,env=dependency_env)
            complete = [json.loads(p.read_text()) for p in app.glob("run-*/report.json") if json.loads(p.read_text())["complete"]]
            require(len(complete) == 1 and len(complete[0]["runs"]) == 5,"incomplete libplacebo acceptance")
            require(complete[0]["sdk"]["revision"] == manifest["revision"] and
                    complete[0]["sdk"]["runtime_sha256"] == manifest["files_sha256"]["lib/libogpu.so"],"libplacebo SDK identity mismatch")
        print("Independent foundation libplacebo source graph, runtime identity and failure report PASS")
    print("Relocated foundation preview source graph, runtime identity and failure handling PASS")


if __name__ == "__main__": main()
