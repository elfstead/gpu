#!/usr/bin/env python3
"""Build and install a revision-identified Linux x86-64 SDK into a NEW prefix."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def run(*args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)


def install(prefix):
    if sys.platform != "linux" or platform.machine() != "x86_64":
        raise ValueError("this installer currently supports native Linux x86-64 only")
    prefix = prefix.absolute()
    if prefix.exists() or prefix.is_symlink():
        raise ValueError("prefix already exists; select a new revision-specific directory")
    if not prefix.parent.is_dir():
        raise ValueError("prefix parent must already exist")
    cargo = os.getenv("CARGO", "cargo")
    # Explicit native target and profile; use Cargo's artifact message rather than
    # assuming target/release or accidentally shipping a stale debug library.
    target = "x86_64-unknown-linux-gnu"
    result = run(cargo, "build", "--locked", "--release", "--target", target,
                 "-p", "ogpu", "--message-format=json-render-diagnostics", cwd=ROOT, capture_output=True)
    print(result.stderr, end="", file=sys.stderr)
    artifacts = [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
    libraries = [Path(path) for item in artifacts if item.get("reason") == "compiler-artifact"
                 and item["target"]["name"] == "ogpu" for path in item["filenames"] if path.endswith(".so")]
    if len(libraries) != 1:
        raise ValueError("Cargo did not identify exactly one ogpu shared library")
    metadata = json.loads(run(cargo, "metadata", "--locked", "--format-version", "1",
                             "--filter-platform", target, cwd=ROOT, capture_output=True).stdout)
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    package = next(p for p in packages.values() if p["name"] == "ogpu")
    dependency_ids = set()

    def dependencies(identity):
        for edge in nodes[identity]["deps"]:
            if not any(k["kind"] is None for k in edge["dep_kinds"]):
                continue
            if edge["pkg"] not in dependency_ids:
                dependency_ids.add(edge["pkg"])
                dependencies(edge["pkg"])
    dependencies(package["id"])
    revision = run("git", "rev-parse", "HEAD", cwd=ROOT, capture_output=True).stdout.strip()
    dirty = bool(run("git", "status", "--porcelain", "--untracked-files=normal", cwd=ROOT,
                     capture_output=True).stdout.strip())
    abi = re.search(r"#define OGPU_ABI_VERSION UINT32_C\((\d+)\)", (ROOT / "include/ogpu.h").read_text())
    if not abi:
        raise ValueError("missing ABI version")
    # Never merge into or overwrite an existing installation. Failed staging is
    # retained for inspection, not recursively removed by this tool.
    staging = Path(tempfile.mkdtemp(prefix=".ogpu-install-", dir=prefix.parent))
    print(f"Staging installation: {staging}", flush=True)

    def copy(source, relative):
        output = staging / relative
        output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, output)

    copy(libraries[0], "lib/libogpu.so")
    copy(ROOT / "include/ogpu.h", "include/ogpu.h")
    copy(ROOT / "LICENSE", "share/ogpu/licenses/ogpu-MIT.txt")
    copy(ROOT / "crates/vulkan-sys/LICENSE-KHRONOS", "share/ogpu/licenses/Vulkan-Headers-MIT.txt")
    dependency_record = []
    for identity in sorted(dependency_ids):
        dependency = packages[identity]
        dependency_record.append({k: dependency[k] for k in ("name", "version", "license")})
        if dependency["source"] is None:
            continue
        directory = Path(dependency["manifest_path"]).parent
        licenses = sorted(p for p in directory.iterdir() if p.is_file()
                          and p.name.upper().startswith(("LICENSE", "COPYING", "NOTICE")))
        if not licenses:
            raise ValueError(f"no license text found for {dependency['name']}; staging retained at {staging}")
        for license_file in licenses:
            copy(license_file, f"share/ogpu/licenses/{dependency['name']}-{dependency['version']}/{license_file.name}")
    copy(ROOT / "examples/compiler/generate.py", "share/ogpu/tools/generate.py")
    copy(ROOT / "examples/compiler/layouts.py", "share/ogpu/tools/layouts.py")
    copy(ROOT / "examples/compiler/heaps.py", "share/ogpu/tools/heaps.py")
    copy(ROOT / "examples/compiler/stages.py", "share/ogpu/tools/stages.py")
    copy(ROOT / "examples/compiler/link_graphics.py", "share/ogpu/tools/link_graphics.py")
    copy(ROOT / "examples/compiler/build_inputs.py", "share/ogpu/tools/build_inputs.py")
    copy(ROOT / "tools/sdk/ogpu-graphics", "bin/ogpu-graphics")
    (staging / "bin/ogpu-graphics").chmod(0o755)
    copy(ROOT / "tools/sdk/ogpu-shader", "bin/ogpu-shader")
    (staging / "bin/ogpu-shader").chmod(0o755)
    for source, destination in (("examples/compiler/consumer.c", "main.c"),
                                ("examples/compiler/transform.slang", "transform.slang"),
                                ("examples/compiler/transform.generated.h", "transform.generated.h"),
                                ("tools/sdk/build-example.py", "build.py")):
        copy(ROOT / source, f"share/ogpu/examples/transform/{destination}")
    for source, destination in (("examples/compiler/affine.c", "main.c"),
                                ("examples/compiler/affine.slang", "affine.slang"),
                                ("examples/compiler/affine.generated.h", "affine.generated.h"),
                                ("tools/sdk/build-example.py", "build.py")):
        copy(ROOT / source, f"share/ogpu/examples/affine/{destination}")
    for source, destination in (("examples/compiler/structured.c", "main.c"),
                                ("examples/compiler/structured.slang", "structured.slang"),
                                ("examples/compiler/structured.generated.h", "structured.generated.h"),
                                ("tools/sdk/build-example.py", "build.py")):
        copy(ROOT / source, f"share/ogpu/examples/structured/{destination}")
    for source, destination in (("examples/heap_image.c", "main.c"),
                                ("examples/shaders/heap-process.slang", "heap-process.slang"),
                                ("examples/shaders/heap-sample.slang", "heap-sample.slang"),
                                ("examples/compiler/heap_process.generated.h", "heap_process.generated.h"),
                                ("examples/compiler/heap_sample.generated.h", "heap_sample.generated.h"),
                                ("examples/shaders/fullscreen.vert.spv", "fullscreen.vert.spv"),
                                ("examples/shaders/image-pattern.frag.spv", "image-pattern.frag.spv"),
                                ("tools/sdk/build-example.py", "build.py")):
        copy(ROOT / source, f"share/ogpu/examples/heap-image/{destination}")
    for source, destination in (("examples/compiler/stage_pair.c", "main.c"),
                                ("examples/compiler/stage_vertex.slang", "stage_vertex.slang"),
                                ("examples/compiler/stage_fragment.slang", "stage_fragment.slang"),
                                ("examples/compiler/pattern.generated.h", "pattern.generated.h"),
                                ("tools/sdk/build-example.py", "build.py")):
        copy(ROOT / source, f"share/ogpu/examples/stage-pair/{destination}")
    for name in ("affine.slang", "affine_math.slang", "include/arguments.slangh", "include/config.slangh",
                 "include/math.slangh", "affine.generated.h", "build.json"):
        copy(ROOT / "examples/compiler/dependencies" / name, "share/ogpu/examples/dependencies/"+name)
    copy(ROOT / "examples/compiler/affine.c", "share/ogpu/examples/dependencies/main.c")
    copy(ROOT / "tools/sdk/build-example.py", "share/ogpu/examples/dependencies/build.py")
    copy(ROOT / "docs/quickstart.md", "share/ogpu/QUICKSTART.md")
    pc = staging / "lib/pkgconfig/ogpu.pc"
    pc.parent.mkdir(parents=True, exist_ok=True)
    pc.write_text("prefix=${pcfiledir}/../..\nlibdir=${prefix}/lib\nincludedir=${prefix}/include\n"
                  f"ogpu_revision={revision}\nogpu_abi={abi[1]}\n"
                  "Name: ogpu\nDescription: Experimental open GPU runtime (revision-pinned)\n"
                  f"Version: {package['version']}\n"
                  'Libs: -L"${libdir}" -logpu\nCflags: -I"${includedir}"\n')
    files = {str(p.relative_to(staging)): hashlib.sha256(p.read_bytes()).hexdigest()
             for p in sorted(staging.rglob("*")) if p.is_file()}
    manifest = dict(schema=1, revision=revision, source_dirty=dirty, abi=int(abi[1]),
                    version=package["version"], target=target, profile="release",
                    rustc=run(os.getenv("RUSTC", "rustc"), "--version", capture_output=True).stdout.strip(),
                    dependencies=dependency_record, files_sha256=files,
                    compatibility="Match header/library/artifacts from one revision; no stable ABI promise.",
                    binary_scope="Locally built; host libc/toolchain dependencies are not bundled.")
    (staging / "share/ogpu/manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    # Check again before publishing. There is no overwrite mode.
    if prefix.exists() or prefix.is_symlink():
        raise ValueError(f"prefix appeared during build; staging retained at {staging}")
    staging.rename(prefix)
    print(f"Installed {revision}{' (dirty source)' if dirty else ''}, ABI {abi[1]} into {prefix}")
    return prefix


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prefix", required=True, type=Path)
    try:
        install(parser.parse_args().prefix)
    except subprocess.CalledProcessError as error:
        print(error.stdout or "", error.stderr or "", file=sys.stderr)
        raise
