"""Source-only native Rust handoff, preserving the checked consumer's path graph."""
import json
import tomllib

BUNDLE = "share/ogpu/native-rust"


def install(root, staging, copy, revision, dirty):
    def put(source, relative):
        copy(root / source, f"{BUNDLE}/{relative}")

    def source_tree(relative):
        for path in sorted((root / relative).rglob("*")):
            if path.is_symlink():
                raise ValueError(f"symlink in Rust source bundle: {path}")
            if path.is_file():
                put(path.relative_to(root), path.relative_to(root))

    for crate in ("ogpu", "vulkan-sys"):
        put(f"crates/{crate}/Cargo.toml", f"crates/{crate}/Cargo.toml")
        source_tree(f"crates/{crate}/src")
    put("crates/vulkan-sys/LICENSE-KHRONOS", "crates/vulkan-sys/LICENSE-KHRONOS")
    for name in ("Cargo.toml", "Cargo.lock", "build.rs", "README.md"):
        put(f"examples/foundation_rust/{name}", f"examples/foundation_rust/{name}")
    source_tree("examples/foundation_rust/src")
    for name in ("foundation-compute.comp.spv", "foundation-compute.slang"):
        put(f"examples/shaders/{name}", f"examples/shaders/{name}")
    for name in ("textured_scene.generated.h", "textured.vert.slang", "textured.frag.slang"):
        put(f"examples/graphics_scene/{name}", f"examples/graphics_scene/{name}")
    put("LICENSE", "LICENSE")
    put("include/ogpu_next.h", "include/ogpu_next.h")
    for name in ("presentation-contract.md", "presentation-acceptance.md",
                 "results/p1-swapchain-lifecycle-2026-10-15.md",
                 "results/p1-interactive-scene-2026-10-16.md"):
        put(f"docs/{name}", f"docs/{name}")
    put("tools/sdk/native-rust/README.md", "README.md")
    # Preserve crate manifests and workspace-inherited package identity without
    # dragging the repository's xtask/build tooling into this source handoff.
    package = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]
    workspace = '[workspace]\nmembers = ["crates/ogpu", "crates/vulkan-sys"]\nresolver = "2"\n\n[workspace.package]\n'
    for name in ("version", "edition", "rust-version", "license"):
        workspace += f"{name} = {json.dumps(package[name])}\n"
    (staging / BUNDLE / "Cargo.toml").write_text(workspace)
    identity = dict(revision=revision, source_dirty=dirty, target="x86_64-unknown-linux-gnu",
                    manifest="examples/foundation_rust/Cargo.toml", default_features=False,
                    registry_dependencies="Cargo.lock pinned; fetch or supply a Cargo cache; not vendored",
                    linkage="native Rust source, not the SDK C shared library")
    (staging / BUNDLE / "source.json").write_text(json.dumps(identity, indent=2) + "\n")
    return dict(path=BUNDLE, **identity)
