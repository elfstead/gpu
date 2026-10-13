#!/usr/bin/env python3
"""Relocate the foundation SDK and check independent public-only consumers."""
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
    if manifest.get('default_api')=='foundation':
        legacy=manifest['legacy_sdk']
        require(('abi' in manifest)==legacy,'incorrect manifest profile')
        for name in ('include/ogpu.h','lib/pkgconfig/ogpu.pc','share/ogpu/examples/transform/main.c'):
            require((installed/name).is_file()==legacy,'legacy packaging mismatch: '+name)
    for relative,digest in manifest["files_sha256"].items():
        require(hashlib.sha256((installed / relative).read_bytes()).hexdigest() == digest,f"installed hash mismatch: {relative}")
    if 'cargo_features' in manifest:
        legacy=manifest['legacy_sdk']
        require(manifest['cargo_default_features'] is False and
                manifest['cargo_features']==(['legacy'] if legacy else []),'incorrect runtime build profile')
        symbols=run(['nm','-D','--defined-only',str(installed/'lib/libogpu.so')],quiet=True)
        exports={line.split()[-1] for line in symbols.splitlines() if line.split()}
        require('ogpu_next_discovery_create' in exports,'foundation runtime missing')
        old={name for name in exports if name.startswith('ogpu_') and not name.startswith('ogpu_next_')}
        require(('ogpu_probe_create' in old) if legacy else not old,'legacy runtime export mismatch')
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
    if manifest.get('default_api')=='foundation' and not manifest['legacy_sdk']:
        absent=subprocess.run([pkg,'--exists','ogpu'],env=env,capture_output=True,text=True,timeout=30)
        require(absent.returncode!=0,'foundation-only package exposed legacy pkg-config name')
    for label in ("compute","graphics","learned-image","indexed-scene","indexed-ranges","textured-scene"):
        app = temporary / ("independent "+label)
        shutil.copytree(prefix / ("share/ogpu/examples/foundation-"+label),app)
        run([sys.executable,"-B","build.py"],cwd=app,env=env)
        binaries=("baseline","consumer","reuse","scopes") if label=="indexed-ranges" else ("consumer","reuse") if label=="indexed-scene" else ("consumer",)
        for binary in binaries:
            dependencies = [Path(p).resolve() for p in shlex.split((app / (binary+".d")).read_text().replace("\\\n",""))[1:]]
            require(prefix / "include/ogpu_next.h" in dependencies,"preview header missing")
            require(all(p.is_relative_to(prefix) or p.is_relative_to(app) for p in dependencies),"source/header escaped installed graph")
            require(not any("vulkan" in p.name.lower() or p.name == "ogpu.h" for p in dependencies),"consumer imported legacy/native headers")
            linked = run(["ldd","./"+binary],cwd=app,env=env)
            selected = next((line.split("=>",1)[1].rsplit(" (",1)[0].strip() for line in linked.splitlines() if line.strip().startswith("libogpu.so =>")),None)
            require(selected is not None and Path(selected).resolve() == prefix / "lib/libogpu.so","wrong runtime resolved")
            symbols=run(['nm','-u',str(app/binary)],quiet=True,env=env)
            require(all(line.split()[-1].startswith('ogpu_next_') for line in symbols.splitlines()
                        if line.split() and line.split()[-1].startswith('ogpu_')),'foundation consumer imported legacy API')
        missing = dict(env,OGPU_VULKAN_LIBRARY="/deliberately-missing-ogpu-loader.so")
        scripted = label in ("learned-image","indexed-scene","indexed-ranges","textured-scene")
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
                require(len(complete) == 1 and len(complete[0]["runs"]) == (9 if label=="indexed-ranges" else 2),"missing completed handoff receipt")
                if label=="indexed-scene":require(len(complete[0]["reuse_runs"])==32,"missing replay matrix")
                if label=="textured-scene":require(all(len(r['checks'])==18 for r in complete[0]['runs']),'missing textured cases')
                if label=="indexed-ranges":
                    require(len(complete[0]['baseline'])==1 and len(complete[0]['baseline'][0]['checks'])==30
                            and len(complete[0]['reuse_runs'])==48,'missing range baseline/replay matrix')
                require(complete[0]["sdk"]["revision"] == manifest["revision"] and
                        complete[0]["sdk"]["runtime_sha256"] == manifest["files_sha256"]["lib/libogpu.so"],"handoff identity mismatch")
        if args.shader_check and label in ("compute","graphics"):
            shader_env = dict(env,SLANGC=os.getenv("SLANGC","slangc"))
            if label == "compute":
                generator = [str(prefix / "bin/ogpu-shader"),"--source","transform.slang","--output",
                             "foundation.generated.h","--build-dir","shader-build","--name","transform","--stage","compute"]
            else:
                generator = [str(prefix / "bin/ogpu-graphics"),"--vertex-source","stage_vertex.slang",
                             "--fragment-source","stage_fragment.slang","--output","foundation-pattern.generated.h",
                             "--build-dir","shader-build","--name","pattern"]
            run([*generator,"--check"],cwd=app,env=shader_env)
            if not manifest.get('legacy_sdk',True):
                rejected=subprocess.run([*generator,'--api','installed','--check'],cwd=app,
                    env=dict(shader_env,SLANGC='/must-not-run-slang'),capture_output=True,text=True,timeout=30)
                require(rejected.returncode!=0 and 'install with --legacy' in rejected.stderr,'absent legacy profile was not rejected before compilation')
            if label == "compute":
                source = app / "transform.slang"; original = source.read_text()
                changed = original.replace("    uint* data;\n    uint count;","    uint count;\n    uint* data;").replace("numthreads(64, 1, 1)","numthreads(32, 1, 1)")
                require(changed != original,"mutation did not change shader"); source.write_text(changed)
                stale = subprocess.run([*generator,"--check"],cwd=app,env=shader_env,capture_output=True,text=True,timeout=120)
                require(stale.returncode != 0 and "stale generated interface" in stale.stderr,"stale header accepted")
                run(generator,cwd=app,env=shader_env)
                run([sys.executable,"-B","build.py"],cwd=app,env=env)
                if not args.no_gpu: run(["./consumer"],cwd=app,env=env)
        if args.shader_check and label=="indexed-ranges":
            shader_env=dict(env,SLANGC=os.getenv("SLANGC","slangc"))
            generator=[str(prefix / "bin/ogpu-shader"),"--api","foundation","--source","frontier.slang",
                       "--output","foundation_scene_range_prepare.generated.h","--build-dir","shader-build",
                       "--name","scene_range_prepare","--stage","compute"]
            pair=[str(prefix / "bin/ogpu-graphics"),"--api","foundation","--vertex-source","identity.vert.slang",
                  "--fragment-source","identity.frag.slang","--output","foundation_scene_identity_pair.generated.h",
                  "--build-dir","identity-build","--name","scene_identity_pair"]
            run([*generator,"--check"],cwd=app,env=shader_env)
            run([*pair,"--check"],cwd=app,env=shader_env)
            source=app / "frontier.slang";original=source.read_text()
            changed=original.replace("float* vertices; uint* indices; uint* draws; uint* control;",
                                     "uint* control; uint* draws; uint* indices; float* vertices;")
            require(changed!=original,"range mutation did not reorder root")
            # Host C remains unchanged: generated named fields, not a guessed
            # byte layout, supply all four addresses in their new positions.
            host_hashes={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in app.glob("*.c")}
            source.write_text(changed)
            stale=subprocess.run([*generator,"--check"],cwd=app,env=shader_env,capture_output=True,text=True,timeout=120)
            require(stale.returncode!=0 and "stale generated interface" in stale.stderr,"stale range header accepted")
            run(generator,cwd=app,env=shader_env)
            run([sys.executable,"-B","build.py"],cwd=app,env=env)
            require(host_hashes=={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in app.glob("*.c")},"range regeneration changed host source")
            if not args.no_gpu:
                previous={p.resolve() for p in app.glob("run-*/report.json")}
                run([sys.executable,"-B","run.py"],cwd=app,env=env)
                receipts=[json.loads(p.read_text()) for p in app.glob("run-*/report.json") if p.resolve() not in previous]
                require(len(receipts)==1 and receipts[0]['complete'] and len(receipts[0]['runs'])==9
                        and len(receipts[0]['reuse_runs'])==48,"incomplete regenerated range matrix")
                before=complete[0];after=receipts[0]
                require(after['sdk']==before['sdk'],"regenerated range runtime identity changed")
                require(after['sources']['foundation_scene_range_prepare.generated.h']!=
                        before['sources']['foundation_scene_range_prepare.generated.h'],"range generated layout did not change")
                for old,new in zip(before['runs'],after['runs']):
                    require((old['extent'],old['capacity'],old['strategy'],old['device'])==
                            (new['extent'],new['capacity'],new['strategy'],new['device']),"regenerated range case changed")
                    require({k:v for k,v in old['files'].items() if k.endswith(('.images','.geometry'))}==
                            {k:v for k,v in new['files'].items() if k.endswith(('.images','.geometry'))},"regenerated range output changed")
            print("Independent range root regeneration preserves host source"+
                  (" BUILD ONLY" if args.no_gpu else " and checked output PASS"),flush=True)
        if args.shader_check and label=='textured-scene':
            shader_env=dict(env,SLANGC=os.getenv('SLANGC','slangc'))
            generator=[str(prefix/'bin/ogpu-graphics'),'--native-heaps','--vertex-source','textured.vert.slang',
                '--fragment-source','textured.frag.slang','--name','textured_scene','--output','textured_scene.generated.h',
                '--build-dir','shader-build']
            run([*generator,'--check'],cwd=app,env=shader_env)
            host_hashes={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in app.glob('*.c')}
            for name in ('textured.vert.slang','textured.frag.slang'):
                source=app/name;original=source.read_text()
                changed=original.replace('float* vertices; uint image; uint samplerIndex; float lod; uint overlay;',
                    'uint overlay; float lod; uint samplerIndex; uint image; float* vertices;')
                require(changed!=original,'texture mutation did not reorder root');source.write_text(changed)
            stale=subprocess.run([*generator,'--check'],cwd=app,env=shader_env,capture_output=True,text=True,timeout=120)
            require(stale.returncode!=0 and 'stale generated graphics pair' in stale.stderr,'stale textured pair accepted')
            run(generator,cwd=app,env=shader_env);run([sys.executable,'-B','build.py'],cwd=app,env=env)
            require(host_hashes=={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in app.glob('*.c')},'texture regeneration changed host source')
            if not args.no_gpu:
                previous={p.resolve() for p in app.glob('run-*/report.json')}
                run([sys.executable,'-B','run.py'],cwd=app,env=env)
                receipts=[json.loads(p.read_text()) for p in app.glob('run-*/report.json') if p.resolve() not in previous]
                require(len(receipts)==1 and receipts[0]['complete'] and len(receipts[0]['runs'])==2,'incomplete regenerated texture matrix')
                before=complete[0];after=receipts[0]
                require(before['sdk']==after['sdk'],'texture runtime identity changed')
                require(before['sources']['textured_scene.generated.h']!=after['sources']['textured_scene.generated.h'],'texture layout did not change')
                for old,new in zip(before['runs'],after['runs']):
                    require(old['extent']==new['extent'] and old['device']==new['device'],'regenerated texture case changed')
                    require({k:v for k,v in old['files'].items() if k.endswith(('.images','.geometry','.texture'))}==
                            {k:v for k,v in new['files'].items() if k.endswith(('.images','.geometry','.texture'))},'regenerated texture outputs changed')
            print('Independent textured root regeneration preserves host source'+(' BUILD ONLY' if args.no_gpu else ' and checked output PASS'),flush=True)
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
