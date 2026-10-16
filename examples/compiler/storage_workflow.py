#!/usr/bin/env python3
"""Compile/check and execute typed/formatless storage images, without device gates."""
import argparse
import os
import shlex
import subprocess
import sys
sys.dont_write_bytecode = True
import generate as g

# Third RWTexture2D generic is pinned Slang's ImageFormat, not a Vulkan/SPIR-V ID.
# Every emitted OpTypeImage format/scalar is checked against this table below.
CASES = (("unorm", "float4", 10, "Rgba8", "RGBA8_UNORM", 4),
         ("sint", "int4", 23, "Rgba8i", "RGBA8_SINT", 4),
         ("uint", "uint4", 33, "Rgba8ui", "RGBA8_UINT", 4),
         ("float", "float2", 3, "Rg32f", "RG32_FLOAT", 8))


def compile_case(build, operation, mode):
    label, scalar, fmt, spirv_format, native, size = CASES[operation]
    folder = build / f"{label}-{mode}"
    folder.mkdir(parents=True, exist_ok=True)
    source = folder / "storage.slang"
    source.write_text(f"#define TEXEL {scalar}\n#define INPUT_FORMAT {0 if mode & 1 else fmt}\n"
                      f"#define OUTPUT_FORMAT {0 if mode & 2 else fmt}\n#define OPERATION {operation}\n"
                      + (g.HERE / "storage.slang").read_text())
    reflection, assembly, binary = g.compile_source(source, folder, os.getenv("SLANGC", "slangc"), native_heaps=True)
    expected = {("Unknown" if mode & 1 else spirv_format), ("Unknown" if mode & 2 else spirv_format)}
    actual = {line.split()[-1] for line in assembly.splitlines() if "OpTypeImage" in line}
    g.require(actual == expected, "pinned compiler image-format mapping changed")
    generated = g.header(reflection, assembly, binary, source.read_bytes(), "storage", "foundation")
    fields, _, _, _, _ = g.inspect(reflection, assembly, "storage")
    flags = set().union(*map(set, fields.image_requirements.values()))
    g.require(("STORAGE_READ_WITHOUT_FORMAT" in flags) == bool(mode & 1), "incorrect read requirement")
    g.require(("STORAGE_WRITE_WITHOUT_FORMAT" in flags) == bool(mode & 2), "incorrect write requirement")
    g.require("storage_required_features (UINT64_C(0))" in generated, "artificial device gate")
    (folder / "storage.generated.h").write_text(generated)
    return folder, reflection, assembly, binary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="compile and check without GPU execution")
    args = parser.parse_args()
    os.chdir(g.ROOT)
    g.require(not os.getenv("CARGO_TARGET_DIR"), "leave CARGO_TARGET_DIR unset")
    build = g.ROOT / "target/compiler-storage"
    subprocess.run([os.getenv("CARGO", "cargo"), "build", "--locked", "-p", "ogpu", "--no-default-features"], check=True)
    library = g.ROOT / "target/debug"
    environment = os.environ.copy()
    environment["LD_LIBRARY_PATH"] = str(library) + os.pathsep + environment.get("LD_LIBRARY_PATH", "")
    executed = unsupported = 0
    for operation, (label, _, _, spirv_format, native, size) in enumerate(CASES):
        for mode in range(4):
            folder, _, _, _ = compile_case(build, operation, mode)
            unknown = "unknown_" + ("float" if label == "unorm" else label)
            requirement = lambda format: f"storage_storage_image_2d_{format}_format_features"
            input_requirement = requirement(unknown if mode & 1 else spirv_format.lower())
            output_requirement = requirement(unknown if mode & 2 else spirv_format.lower())
            executable = folder / "consumer"
            subprocess.run([*shlex.split(os.getenv("CC", "cc")), "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror",
                            "-I", str(folder), "-Iinclude", "examples/compiler/storage.c",
                            f"-DIMAGE_FORMAT=OGPU_NEXT_{native}", f"-DTEXEL_BYTES={size}",
                            f"-DOPERATION={operation}", f"-DFORMATLESS_MODE={mode}", "-L", str(library),
                            f"-DINPUT_REQUIREMENTS={input_requirement}", f"-DOUTPUT_REQUIREMENTS={output_requirement}",
                            f"-Wl,-rpath,{library}", "-logpu", "-o", str(executable)], check=True)
            if not args.check:
                result = subprocess.run([str(executable)], capture_output=True, text=True, env=environment)
                print(result.stdout, end=""); print(result.stderr, end="", file=sys.stderr)
                g.require(result.returncode == 0 and "Validation Error" not in result.stdout + result.stderr,
                          f"storage consumer failed: {folder.name}")
                g.require(("PASS" in result.stdout) != ("UNSUPPORTED" in result.stdout), "missing/ambiguous disposition")
                executed += "PASS" in result.stdout
                unsupported += "UNSUPPORTED" in result.stdout
    subprocess.run([sys.executable, "-B", str(g.HERE / "test_storage.py")], check=True)
    print(f"Storage image workflow: 16 compiled; {executed} executed, {unsupported} unsupported; "
          + ("host-only checks" if args.check else "GPU read/write/copy oracle") + " PASS")


if __name__ == "__main__": main()
