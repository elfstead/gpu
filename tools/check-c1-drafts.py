#!/usr/bin/env python3
"""Check uninstalled C1 records; no runtime or GPU acceptance."""

import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
ARGUMENT_RECORDS = [
    ("execution_limits", "ExecutionLimits", "max_inline_bytes max_groups max_local_size max_local_invocations max_shared_memory argument_kinds"),
    ("shader", "Shader", "stage format code entry specialization subgroup"),
    ("executable_desc", "ExecutableDesc", "header kind shader_count shaders static_state dynamic_state requirements cache compile_flags reserved"),
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
    ("descriptor_limits", "DescriptorLimits", "max_resource_capacity max_sampler_capacity uniform_address_alignment storage_address_alignment max_uniform_range max_storage_range max_sampler_lod_bias max_sampler_anisotropy"),
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
PRESENTATION_RECORDS = [
    ("win32_surface_desc", "Win32SurfaceDesc", "header hwnd"),
    ("dxgi_chain_desc", "DxgiChainDesc",
     "header surface queue format extent image_count usage swap_effect scaling alpha color_space chain_flags maximum_frame_latency"),
    ("presentation_contract", "PresentationContract",
     "profile availability capabilities max_present_count"),
    ("chain_info", "ChainInfo", "generation state image_count ordered_queue"),
    ("chain_create_outcome", "ChainCreateOutcome", "chain previous_state reserved"),
    ("chain_resize_desc", "ChainResizeDesc", "format extent image_count chain_flags reserved"),
    ("chain_resize_outcome", "ChainResizeOutcome", "generation state image_effect"),
    ("acquire_desc", "AcquireDesc", "timeout_ns binary fence"),
    ("frame", "Frame", "chain image generation index availability ordered_queue binary fence"),
    ("present_item", "PresentItem", "frame retire_fence sync_interval native_flags"),
    ("present_result", "PresentResult", "status frame_effect fence_effect reserved"),
    ("present_desc", "PresentDesc", "wait_count item_count waits items results scratch"),
    ("present_outcome", "PresentOutcome", "work_effect wait_effect"),
    ("pacing_handle", "PacingHandle", "kind reserved handle"),
]
PRESENTATION_CONSTANTS = """PRESENT_VULKAN PRESENT_DXGI PRESENTATION_WIN32 OCCLUDED STILL_DRAWING
WIN32_SURFACE_DESC DXGI_CHAIN_DESC
AVAILABILITY_SIGNALS AVAILABILITY_QUEUE_ORDERED PRESENT_REPLACE PRESENT_RESIZE
PRESENT_RETIRE_FENCE PRESENT_PACING_HANDLE PRESENT_BINARY_WAIT CHAIN_NONE CHAIN_ACTIVE
CHAIN_RETIRED CHAIN_NO_IMAGES CHAIN_LOST IMAGES_UNCHANGED IMAGES_REPLACED IMAGES_INVALIDATED
FRAME_RETAINED FRAME_RELEASED FRAME_UNKNOWN EFFECT_NONE EFFECT_ENQUEUED EFFECT_UNKNOWN
WAITABLE_WIN32""".split()
DEPENDENCY_RECORDS = [
    ("dependency_limits", "DependencyLimits", "domain flags memory_scope reserved wait_stages signal_stages split_stages"),
    ("barrier_scope", "BarrierScope", "stages_before stages_after access_before access_after"),
    ("memory_barrier", "MemoryBarrier", "scope range source_domain destination_domain"),
    ("image_barrier", "ImageBarrier", "scope image range old_state new_state source_domain destination_domain discard reserved"),
    ("dependency", "Dependency", "global_count memory_count image_count flags global memory images"),
    ("split_dependency", "SplitDependency", "split dependency"),
    ("submit_desc", "SubmitDesc", "list_count wait_count signal_count binary_wait_count binary_signal_count flags lists waits signals binary_waits binary_signals"),
    ("submit_outcome", "SubmitOutcome", "status acceptance"),
]
DEPENDENCY_CONSTANTS = """QUERY_DEPENDENCY_LIMITS DEPENDENCY_SPLIT DEPENDENCY_OWNERSHIP
DEPENDENCY_BINARY MEMORY_SCOPE_NONE MEMORY_SCOPE_OBJECT MEMORY_SCOPE_BYTES
SUBMIT_REJECTED SUBMIT_ACCEPTED SUBMIT_UNKNOWN""".split()
MINIMAL_RECORDS = [
    ("inheritance", "Inheritance", "header occlusion query_flags statistics rendering resource_heap sampler_heap"),
    ("generated_layout_desc", "GeneratedLayoutDesc", "header arguments stages flags stride token_count reserved tokens"),
]
MINIMAL_CONSTANTS = ["INHERITANCE_DESC", "GENERATED_ARGUMENT"]


def main():
    cc = os.environ.get("CC", "cc")
    rustc = os.environ.get("RUSTC", "rustc")
    headers = ["ogpu_arguments.h", "ogpu_descriptors.h", "ogpu_presentation.h", "ogpu_dependencies.h", "ogpu_minimal.h"]
    c = ['#include <stdio.h>', '#include <stddef.h>',
         *[f'#include "docs/drafts/{h}"' for h in headers], 'int main(void) {']
    rust = ['mod arguments;', 'mod descriptors;', 'mod presentation;', 'mod dependencies;', 'mod minimal;',
            'use std::mem::{size_of, align_of, offset_of};', 'fn main() {']
    records = [("ogpu_draft_" + name, f"arguments::{rn}", fields)
               for name, rn, fields in ARGUMENT_RECORDS]
    records += [("ogpu_draft_" + name, f"descriptors::{rn}", fields)
                for name, rn, fields in DESCRIPTOR_RECORDS]
    records += [("ogpu_draft_" + name, f"presentation::{rn}", fields)
                for name, rn, fields in PRESENTATION_RECORDS]
    records += [("ogpu_draft_" + name, f"dependencies::{rn}", fields)
                for name, rn, fields in DEPENDENCY_RECORDS]
    records += [("ogpu_draft_" + name, f"minimal::{rn}", fields)
                for name, rn, fields in MINIMAL_RECORDS]
    # Verify imported layouts too: descriptor drafts mirror borrowed spans.
    records += [("ogpu_next_span", "descriptors::Span", "memory offset size"),
                ("ogpu_next_host_span", "descriptors::HostSpan", "data size"),
                ("ogpu_next_record", "presentation::Record", "kind version byte_size flags next"),
                ("ogpu_next_point", "dependencies::Point", "timeline value"),
                ("ogpu_next_sync_point", "dependencies::SyncPoint", "point stages"),
                ("ogpu_next_binary_point", "dependencies::BinaryPoint", "binary stages"),
                ("ogpu_next_subresources", "dependencies::Subresources", "aspects first_mip mip_count first_layer layer_count"),
                ("ogpu_next_bytes", "arguments::Bytes", "data size"),
                ("ogpu_next_generated_token", "minimal::GeneratedToken", "kind offset argument_offset argument_size")]
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
    constants += [(name, "presentation::" + name) for name in PRESENTATION_CONSTANTS]
    constants += [(name, "dependencies::" + name) for name in DEPENDENCY_CONSTANTS]
    constants += [(name, "minimal::" + name) for name in MINIMAL_CONSTANTS]
    for name, rt in constants:
        c.append(f'printf("{name}=%llu\\n", (unsigned long long)OGPU_DRAFT_{name});')
        rust.append(f'println!("{name}={{}}", {rt});')
    c.append("return 0; }")
    rust.append("}")
    with tempfile.TemporaryDirectory(prefix="ogpu-c1-draft-") as directory:
        temp = Path(directory)
        (temp / "check.c").write_text("\n".join(c))
        (temp / "check.rs").write_text("\n".join(rust))
        for module in ("arguments", "descriptors", "presentation", "dependencies", "minimal"):
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
