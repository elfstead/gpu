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


def install(prefix, *, legacy=False):
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
    features = ["legacy"] if legacy else []
    feature_args = ["--no-default-features"] + (["--features", "legacy"] if legacy else [])
    result = run(cargo, "build", "--locked", "--release", "--target", target,
                 "-p", "ogpu", *feature_args, "--message-format=json-render-diagnostics", cwd=ROOT, capture_output=True)
    print(result.stderr, end="", file=sys.stderr)
    artifacts = [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
    libraries = [Path(path) for item in artifacts if item.get("reason") == "compiler-artifact"
                 and item["target"]["name"] == "ogpu" for path in item["filenames"] if path.endswith(".so")]
    if len(libraries) != 1:
        raise ValueError("Cargo did not identify exactly one ogpu shared library")
    metadata = json.loads(run(cargo, "metadata", "--locked", "--format-version", "1",
                             "--filter-platform", target, *feature_args, cwd=ROOT, capture_output=True).stdout)
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
    copy(ROOT / "include/ogpu_next.h", "include/ogpu_next.h")
    if legacy:
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
    copy(ROOT / "tools/sdk/profile.py", "share/ogpu/tools/sdk_profile.py")
    copy(ROOT / "tools/sdk/ogpu-graphics", "bin/ogpu-graphics")
    (staging / "bin/ogpu-graphics").chmod(0o755)
    copy(ROOT / "tools/sdk/ogpu-shader", "bin/ogpu-shader")
    (staging / "bin/ogpu-shader").chmod(0o755)
    if legacy:
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
        for source, destination in (("examples/compiler/ranges.c", "main.c"),
                                    ("examples/graphics_scene/identity.vert.slang", "identity.vert.slang"),
                                    ("examples/graphics_scene/identity.frag.slang", "identity.frag.slang"),
                                    ("examples/compiler/identity.generated.h", "identity.generated.h"),
                                    ("examples/compiler/range_count.slang", "range_count.slang"),
                                    ("examples/compiler/range_count.generated.h", "range_count.generated.h"),
                                    ("tools/sdk/build-example.py", "build.py")):
            copy(ROOT / source, f"share/ogpu/examples/draw-ranges/{destination}")
        for name in ("affine.slang", "affine_math.slang", "include/arguments.slangh", "include/config.slangh",
                     "include/math.slangh", "affine.generated.h", "build.json"):
            copy(ROOT / "examples/compiler/dependencies" / name, "share/ogpu/examples/dependencies/"+name)
        for name in ("public.c", "prepare.slang", "prepare16.slang", "scene.vert.slang", "scene.frag.slang",
                     "scene_prepare.generated.h", "scene_prepare16.generated.h", "scene_pair.generated.h", "oracle.py"):
            copy(ROOT / "examples/graphics_scene" / name, "share/ogpu/examples/indexed-scene/"+name)
        copy(ROOT / "examples/graphics_scene/generated.c", "share/ogpu/examples/indexed-scene/main.c")
        copy(ROOT / "tools/sdk/build-example.py", "share/ogpu/examples/indexed-scene/build.py")
        for name in ("run.py", "README.md"):
            copy(ROOT / "tools/sdk/indexed-scene" / name, "share/ogpu/examples/indexed-scene/"+name)
        copy(ROOT / "examples/compiler/affine.c", "share/ogpu/examples/dependencies/main.c")
        copy(ROOT / "tools/sdk/build-example.py", "share/ogpu/examples/dependencies/build.py")
    copy(ROOT / "docs/quickstart.md", "share/ogpu/QUICKSTART.md")
    for name in ("release-policy.md", "support.md", "foundation-quickstart.md", "legacy-sdk.md"):
        copy(ROOT / "docs" / name, f"share/ogpu/{name}")
    if legacy:
        # Keep the measured consumer's relative source graph intact. Only the public
        # branch is built; no Vulkan headers/native backend, checkout or target assets.
        reuse = "share/ogpu/examples/resource-reuse"
        for name in ("main.c", "build.py", "run.py", "README.md"):
            copy(ROOT / "tools/sdk/resource-reuse" / name, f"{reuse}/{name}")
        for relative in ("performance_frontier/stream.c", "performance_frontier/small.c",
                         "resource_reuse/ranges.h", "resource_reuse/slots.h", "resource_reuse/stream_ranges.h",
                         "learned_image/extent.h", "compiler/transform.generated.h"):
            copy(ROOT / "examples" / relative, f"{reuse}/support/{relative}")
        for name in ("application", "hidden", "denoise", "process", "poison", "fullscreen", "display"):
            copy(ROOT / f"examples/learned_image/generated/{name}.generated.h",
                 f"{reuse}/{name}.generated.h")
        for name in ("reference.py", "model.json"):
            copy(ROOT / "examples/learned_image" / name, f"{reuse}/{name}")
    for name in ("foundation-ggml.md","foundation-libplacebo.md","foundation-indexed-scene.md","foundation-scene-reuse.md","foundation-indexed-ranges.md"):
        copy(ROOT / "docs" / name,f"share/ogpu/{name}")
    for label, files in {
        "foundation-compute": [("examples/compiler/foundation.c","main.c"),
            ("examples/compiler/transform.slang","transform.slang"),
            ("examples/compiler/foundation.generated.h","foundation.generated.h")],
        "foundation-graphics": [("examples/compiler/foundation_graphics.c","main.c"),
            ("examples/compiler/stage_vertex.slang","stage_vertex.slang"),
            ("examples/compiler/stage_fragment.slang","stage_fragment.slang"),
            ("examples/compiler/foundation-pattern.generated.h","foundation-pattern.generated.h")],
        "foundation-indexed-scene": [("examples/graphics_scene/foundation_generated.c","main.c"),
            ("examples/graphics_scene/reuse_foundation_generated.c","reuse_main.c"),
            *[("examples/graphics_scene/"+n,n) for n in ("foundation.c","oracle.py",
                "reuse_foundation.c","reuse.h","foundation_scene_reuse_prepare.generated.h","foundation_scene_reuse_prepare16.generated.h",
                "foundation_scene_prepare.generated.h","foundation_scene_prepare16.generated.h","foundation_scene_pair.generated.h")],
            ("tools/sdk/indexed-scene/run.py","scene_checks.py"),
            ("tools/sdk/foundation-scene/run.py","run.py"),
            ("tools/sdk/foundation-scene/README.md","README.md")],
        "foundation-indexed-ranges": [("examples/graphics_scene/foundation_generated.c","baseline.c"),
            ("examples/graphics_scene/frontier_foundation_generated.c","main.c"),
            ("examples/graphics_scene/range_foundation_generated.c","reuse_main.c"),
            *[("examples/graphics_scene/"+n,n) for n in ("foundation.c","frontier_foundation.c","reuse_foundation.c","range_reuse.h",
                "oracle.py","range_oracle.py","foundation_scene_prepare.generated.h","foundation_scene_prepare16.generated.h",
                "foundation_scene_pair.generated.h","foundation_scene_range_prepare.generated.h","foundation_scene_identity_pair.generated.h",
                "prepare.slang","prepare16.slang","scene.vert.slang","scene.frag.slang","frontier.slang","identity.vert.slang","identity.frag.slang")],
            ("tools/sdk/indexed-scene/run.py","scene_checks.py"),
            ("tools/sdk/foundation-ranges/run.py","run.py"),
            ("tools/sdk/foundation-ranges/README.md","README.md")],
        "foundation-learned-image": [("examples/learned_image/foundation.c","main.c"),
            *[("examples/learned_image/"+n,n) for n in ("extent.h","foundation_timing.h","reference.py","model.json")],
            *[(f"examples/learned_image/generated-foundation/{n}.generated.h",f"{n}.generated.h")
              for n in ("application","hidden","denoise","process","poison","fullscreen","display")],
            ("tools/sdk/foundation/run.py","run.py")],
    }.items():
        for source,destination in files:
            copy(ROOT / source,f"share/ogpu/examples/{label}/{destination}")
        builder="tools/sdk/foundation-scene/build.py" if label=="foundation-indexed-scene" else "tools/sdk/foundation/build.py"
        if label=="foundation-indexed-ranges":builder="tools/sdk/foundation-ranges/build.py"
        copy(ROOT / builder,f"share/ogpu/examples/{label}/build.py")
    ggml = "share/ogpu/examples/foundation-ggml"
    for name in ("CMakeLists.txt","backend.cpp","backend.h","runtime-foundation.h","runtime-installed.h",
                 "mnist.cpp","lifecycle.cpp","matrix-check.cpp","foundation-check.cpp","convert-weights.cpp",
                 "dataset.sha256","fixtures/mnist-fc-f32.gguf","fixtures/SHA256SUMS","fixtures/README.md",
                 "shaders/element.comp.spv","shaders/matrix.comp.spv","shaders/matrix-f16.comp.spv"):
        copy(ROOT / "integrations/ggml" / name,f"{ggml}/{name}")
    for name in ("build.py","run.py","README.md"):
        copy(ROOT / "tools/sdk/ggml" / name,f"{ggml}/{name}")
    placebo = "share/ogpu/examples/foundation-libplacebo"
    for name in ("backend.c","backend.h","backend-tests.c","consumer.c","stream.c","perf.c",
                 "reference.c","hdr.c","compare-hdr.c","compare-reference.cpp","workload.h",
                 "compiler.cpp","compiler.h","metadata.cpp","shader-metadata.hpp","heap-lower.hpp","vertex-pull.hpp"):
        copy(ROOT / "integrations/libplacebo" / name,f"{placebo}/{name}")
    for name in ("build.py","run.py","README.md"):
        copy(ROOT / "tools/sdk/libplacebo" / name,f"{placebo}/{name}")
    pc = staging / "lib/pkgconfig/ogpu.pc"
    pc.parent.mkdir(parents=True, exist_ok=True)
    if legacy:
        pc.write_text("prefix=${pcfiledir}/../..\nlibdir=${prefix}/lib\nincludedir=${prefix}/include\n"
                      f"ogpu_revision={revision}\nogpu_abi={abi[1]}\n"
                      "Name: ogpu\nDescription: Experimental open GPU runtime (revision-pinned)\n"
                      f"Version: {package['version']}\n"
                      'Libs: -L"${libdir}" -logpu\nCflags: -I"${includedir}"\n')
    match = re.search(r"#define OGPU_NEXT_VERSION (\d+)u",(ROOT / "include/ogpu_next.h").read_text())
    if not match: raise ValueError("missing foundation revision")
    foundation_revision = int(match[1])
    (pc.parent / "ogpu-foundation.pc").write_text(
        "prefix=${pcfiledir}/../..\nlibdir=${prefix}/lib\nincludedir=${prefix}/include\n"
        f"ogpu_revision={revision}\nogpu_foundation={foundation_revision}\n"
        "Name: ogpu-foundation\nDescription: Experimental revision-pinned foundation preview\n"
        f"Version: {package['version']}\n"
        'Libs: -L"${libdir}" -logpu\nCflags: -I"${includedir}"\n')
    files = {str(p.relative_to(staging)): hashlib.sha256(p.read_bytes()).hexdigest()
             for p in sorted(staging.rglob("*")) if p.is_file()}
    manifest = dict(schema=1, revision=revision, source_dirty=dirty, default_api="foundation",
                    version=package["version"], target=target, profile="release",
                    cargo_default_features=False, cargo_features=features,
                    rustc=run(os.getenv("RUSTC", "rustc"), "--version", capture_output=True).stdout.strip(),
                    dependencies=dependency_record, files_sha256=files,
                    compatibility="Match header/library/artifacts from one revision; no stable ABI promise.",
                    binary_scope="Locally built; host libc/toolchain dependencies are not bundled.")
    if legacy: manifest["abi"] = int(abi[1])
    manifest["legacy_sdk"] = legacy
    manifest["foundation"] = dict(revision=foundation_revision,compatibility="Exact source revision only; mutable preview, not a stable ABI.")
    (staging / "share/ogpu/manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    # Check again before publishing. There is no overwrite mode.
    if prefix.exists() or prefix.is_symlink():
        raise ValueError(f"prefix appeared during build; staging retained at {staging}")
    staging.rename(prefix)
    print(f"Installed {revision}{' (dirty source)' if dirty else ''}, foundation {foundation_revision}{' + legacy ABI '+abi[1] if legacy else ''} into {prefix}")
    return prefix


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prefix", required=True, type=Path)
    parser.add_argument("--foundation",action="store_true",help=argparse.SUPPRESS)
    parser.add_argument("--legacy",action="store_true",help="also build ABI20 runtime and package its header, examples and ogpu.pc")
    try:
        args = parser.parse_args()
        install(args.prefix,legacy=args.legacy)
    except subprocess.CalledProcessError as error:
        print(error.stdout or "", error.stderr or "", file=sys.stderr)
        raise
