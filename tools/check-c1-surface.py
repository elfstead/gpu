#!/usr/bin/env python3
"""Audit the selected C1 operation inventory against its pinned SDK baseline and C2 coordinated cutover.

This checks operation coverage, not runtime/native correctness or ABI compatibility.
The pinned original inventory remains immutable; C2 tranches explicitly update the
current installed-operation check below.
"""
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def strip_comments(text):
    return re.sub(r"/\*.*?\*/|//[^\n]*", "", text, flags=re.S)

def source(path):
    return strip_comments(path.read_text())


def operation_names(text, prefix):
    # Exclude the return type in `ogpu_next_status (*table_member)(...)`.
    return set(re.findall(rf"\b({prefix}\w+)\s*\((?!\s*\*)", text))


def require(condition, message):
    if not condition:
        raise SystemExit(message)


def main():
    manifest = json.loads((ROOT / "docs/drafts/minimal-core-migration.json").read_text())
    header = ROOT / manifest["baseline_header"]
    require(manifest["design_only"] is True, "Manifest must remain design-only until cutover")
    baseline = subprocess.check_output(["git", "show", manifest["baseline_revision"]+":"+manifest["baseline_header"]], cwd=ROOT)
    require(hashlib.sha256(baseline).hexdigest() == manifest["baseline_sha256"],
            "SDK baseline changed: review the C1 inventory or retire this audit at C2 cutover")
    text = strip_comments(baseline.decode())
    installed = operation_names(text, "ogpu_next_")
    table = re.search(r"typedef struct ogpu_next_presentation_functions \{(.*?)\}", text, re.S)
    require(table is not None, "Presentation table missing from baseline")
    installed.update("presentation." + n for n in re.findall(r"\(\*(\w+)\)\s*\(", table.group(1)))
    retained = set(manifest["retained"])
    replaced = set(manifest["replaced"])
    require(len(retained) == len(manifest["retained"]), "Duplicate retained operation")
    require(not retained & replaced, "Operation both retained and replaced")
    require(installed == retained | replaced,
            f"Unclassified: {sorted(installed - retained - replaced)}; stale: {sorted((retained | replaced) - installed)}")
    drafts = set()
    for name in manifest["draft_headers"]:
        names = operation_names(source(ROOT / name), "ogpu_draft_")
        require(not drafts & names, f"Duplicate draft declaration in {name}")
        drafts.update(names)
    additions = set(manifest["added"])
    replacements = set(manifest["replaced"].values())
    require(len(additions) == len(manifest["added"]), "Duplicate added operation")
    require(not additions & replacements, "Addition also classified as replacement")
    require(drafts == additions | replacements, "Draft declarations and migration destinations differ")
    require(drafts == set(manifest["targets"]), "Missing C/Rust operation correspondence")
    c_targets = [entry["c"] for entry in manifest["targets"].values()]
    rust_targets = [entry["rust"] for entry in manifest["targets"].values()]
    require(len(set(c_targets)) == len(c_targets), "Duplicate target C operation")
    require(len(set(rust_targets)) == len(rust_targets), "Duplicate target Rust operation")
    require(not retained & set(c_targets), "New operation collides with a retained entry")
    require(all(re.fullmatch(r"(?:ogpu_next_|presentation\.)\w+", n) for n in c_targets),
            "Invalid target C name")
    require(all(re.fullmatch(r"[a-z][a-z0-9_]*", n) for n in rust_targets), "Invalid target Rust name")
    # The complete selected surface is now installed. Keep the original design
    # inventory audited against its pinned source until the full C2 deletion audit.
    current = operation_names(source(header), "ogpu_next_")
    current.update("presentation." + n for n in re.findall(r"\(\*(\w+)\)\s*\(",
        re.search(r"typedef struct ogpu_next_presentation_functions \{(.*?)\}", source(header), re.S).group(1)))
    require(current == retained | set(c_targets), "C2 installed operation inventory drift")
    print(f"{len(current)} current operations match the complete selected C2 surface; superseded entry points removed.")
    print(f"{len(installed)} baseline operations covered: {len(retained)} retained, {len(replaced)} replaced.")
    print(f"{len(drafts)} draft operations have exact destinations and C/Rust names; no unclassified entries.")


if __name__ == "__main__":
    main()
