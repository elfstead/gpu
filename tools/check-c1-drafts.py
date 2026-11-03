#!/usr/bin/env python3
"""Check uninstalled C1 records; no runtime or GPU acceptance."""

import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
ARGUMENT_RECORDS = [
    ("argument_field", "ArgumentField",
     "offset size kind address_alignment stages space binding"),
    ("argument_layout_desc", "ArgumentLayoutDesc",
     "execution_class byte_size field_count flags fields"),
    ("argument_state_info", "ArgumentStateInfo",
     "domain binding_key offset_base byte_size bind_rule reserved"),
]
ARGUMENT_CONSTANTS = """INLINE UNIFORM_ADDRESS READ_ADDRESS READ_WRITE_ADDRESS COMPUTE
GRAPHICS DIRECT BIND_PRESERVES BIND_INVALIDATES_ON_CHANGE""".split()
DESCRIPTOR_RECORDS = [
    ("heap_request", "HeapRequest", "kind storage usage flags capacity"),
    ("heap_requirements", "HeapRequirements",
     "unit_bytes backing_size backing_alignment reserved_size reserved_alignment memory_usage"),
    ("heap_desc", "HeapDesc", "request memory host reserved_offset reserved_size"),
    ("descriptor_shape", "DescriptorShape", "size alignment array_stride"),
    ("heap_location", "HeapLocation", "heap offset"),
    ("descriptor_copy", "DescriptorCopy", "source destination size"),
]
DESCRIPTOR_CONSTANTS = """HEAP_HOST HEAP_MEMORY HEAP_NATIVE HEAP_SHADER_VISIBLE
HEAP_CPU_WRITE HEAP_CPU_COPY_SOURCE HEAP_CPU_COPY_DESTINATION
DESCRIPTOR_WRITE_RESOURCE DESCRIPTOR_WRITE_SAMPLER DESCRIPTOR_COPY""".split()


def main():
    cc = os.environ.get("CC", "cc")
    rustc = os.environ.get("RUSTC", "rustc")
    headers = ["ogpu_arguments.h", "ogpu_descriptors.h"]
    c = ['#include <stdio.h>', '#include <stddef.h>',
         *[f'#include "docs/drafts/{h}"' for h in headers], 'int main(void) {']
    rust = ['mod arguments;', 'mod descriptors;',
            'use std::mem::{size_of, align_of, offset_of};', 'fn main() {']
    records = [("ogpu_draft_" + name, f"arguments::{rn}", fields)
               for name, rn, fields in ARGUMENT_RECORDS]
    records += [("ogpu_draft_" + name, f"descriptors::{rn}", fields)
                for name, rn, fields in DESCRIPTOR_RECORDS]
    # Verify imported layouts too: descriptor drafts mirror borrowed spans.
    records += [("ogpu_next_span", "descriptors::Span", "memory offset size"),
                ("ogpu_next_host_span", "descriptors::HostSpan", "data size")]
    for ct, rt, fields in records:
        values = [("size", f"sizeof({ct})", f"size_of::<{rt}>()"),
                  ("align", f"_Alignof({ct})", f"align_of::<{rt}>()")]
        values += [(field, f"offsetof({ct}, {field})", f"offset_of!({rt}, {field})")
                   for field in fields.split()]
        for label, c_expr, rust_expr in values:
            key = f"{ct}.{label}"
            c.append(f'printf("{key}=%zu\\n", {c_expr});')
            rust.append(f'println!("{key}={{}}", {rust_expr});')
    constants = [("ARGUMENT_" + name, "arguments::ARGUMENT_" + name)
                 for name in ARGUMENT_CONSTANTS]
    constants += [(name, "descriptors::" + name) for name in DESCRIPTOR_CONSTANTS]
    for name, rt in constants:
        c.append(f'printf("{name}=%llu\\n", (unsigned long long)OGPU_DRAFT_{name});')
        rust.append(f'println!("{name}={{}}", {rt});')
    c.append("return 0; }")
    rust.append("}")
    with tempfile.TemporaryDirectory(prefix="ogpu-c1-draft-") as directory:
        temp = Path(directory)
        (temp / "check.c").write_text("\n".join(c))
        (temp / "check.rs").write_text("\n".join(rust))
        for module in ("arguments", "descriptors"):
            (temp / f"{module}.rs").write_bytes((ROOT / f"docs/drafts/{module}.rs").read_bytes())
        flags = ["-Wall", "-Wextra", "-Werror", "-pedantic"]
        subprocess.run([cc, "-std=c11", *flags, "-I", str(ROOT),
                        str(temp / "check.c"), "-o", str(temp / "check-c")], check=True)
        for header in headers:
            subprocess.run([cc, "-x", "c++", "-std=c++17", *flags, "-fsyntax-only",
                            str(ROOT / "docs/drafts" / header)], check=True)
        subprocess.run([rustc, "--edition=2024", "-A", "dead_code", "-C", f"linker={cc}",
                        str(temp / "check.rs"), "-o", str(temp / "check-rust")], check=True)
        c_result = subprocess.check_output([str(temp / "check-c")], text=True)
        rust_result = subprocess.check_output([str(temp / "check-rust")], text=True)
        if c_result != rust_result:
            raise SystemExit(f"Record mismatch:\nC:\n{c_result}\nRust:\n{rust_result}")
        print(f"C11/C++17 syntax passed; {len(c_result.splitlines())} C/Rust "
              "size/alignment/offset/constant values match on this host.")
        print(subprocess.check_output([rustc, "--version"], text=True).strip())


if __name__ == "__main__":
    main()
