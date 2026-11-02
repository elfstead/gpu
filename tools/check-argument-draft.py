#!/usr/bin/env python3
"""Check the uninstalled C1 argument records; no runtime or GPU acceptance."""

import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
RECORDS = [
    ("argument_field", "ArgumentField",
     "offset size kind address_alignment stages space binding"),
    ("argument_layout_desc", "ArgumentLayoutDesc",
     "execution_class byte_size field_count flags fields"),
    ("argument_state_info", "ArgumentStateInfo",
     "domain binding_key offset_base byte_size bind_rule reserved"),
]
CONSTANTS = """INLINE UNIFORM_ADDRESS READ_ADDRESS READ_WRITE_ADDRESS COMPUTE
GRAPHICS DIRECT BIND_PRESERVES BIND_INVALIDATES_ON_CHANGE""".split()


def main():
    cc = os.environ.get("CC", "cc")
    rustc = os.environ.get("RUSTC", "rustc")
    c = ['#include <stdio.h>', '#include <stddef.h>',
         '#include "docs/drafts/ogpu_arguments.h"', 'int main(void) {']
    rust = ['#[path="arguments.rs"] mod draft;',
            'use std::mem::{size_of, align_of, offset_of};', 'fn main() {']
    for suffix, rust_name, fields in RECORDS:
        ct, rt = "ogpu_draft_" + suffix, "draft::" + rust_name
        values = [("size", f"sizeof({ct})", f"size_of::<{rt}>()"),
                  ("align", f"_Alignof({ct})", f"align_of::<{rt}>()")]
        values += [(field, f"offsetof({ct}, {field})", f"offset_of!({rt}, {field})")
                   for field in fields.split()]
        for label, c_expr, rust_expr in values:
            key = f"{suffix}.{label}"
            c.append(f'printf("{key}=%zu\\n", {c_expr});')
            rust.append(f'println!("{key}={{}}", {rust_expr});')
    for name in CONSTANTS:
        c.append(f'printf("{name}=%llu\\n", '
                 f'(unsigned long long)OGPU_DRAFT_ARGUMENT_{name});')
        rust.append(f'println!("{name}={{}}", draft::ARGUMENT_{name});')
    c.append("return 0; }")
    rust.append("}")
    with tempfile.TemporaryDirectory(prefix="ogpu-argument-draft-") as directory:
        temp = Path(directory)
        (temp / "check.c").write_text("\n".join(c))
        (temp / "check.rs").write_text("\n".join(rust))
        (temp / "arguments.rs").write_bytes((ROOT / "docs/drafts/arguments.rs").read_bytes())
        flags = ["-Wall", "-Wextra", "-Werror", "-pedantic"]
        subprocess.run([cc, "-std=c11", *flags, "-I", str(ROOT),
                        str(temp / "check.c"), "-o", str(temp / "check-c")], check=True)
        subprocess.run([cc, "-x", "c++", "-std=c++17", *flags, "-fsyntax-only",
                        str(ROOT / "docs/drafts/ogpu_arguments.h")], check=True)
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
