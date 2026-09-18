#!/usr/bin/env python3
"""Export complete native/OGPU correctness evidence, verifying retained local outputs."""
import argparse
import json
from pathlib import Path
import sys
sys.dont_write_bytecode = True
import benchmark as bench
import run
import run_native


def export(report_path, destination):
    report = json.loads(report_path.read_text())
    run.require(report.get("complete") is True and report.get("runs"), "incomplete native workload report")
    rows = report["runs"]
    small = (65, 47, 131, 95)
    extents = {tuple(r["extent"]) for r in rows}
    run.require(extents == {small} or extents == {small, *bench.EXTENTS}, "incomplete extent matrix")
    expected = {(extent, engine, variant, mode) for extent in extents
                for engine in ("native", "ogpu") for variant in (("original", "mutated") if extent == small else ("original",))
                for mode in bench.MODES}
    keys = [(tuple(r["extent"]), r["engine"], r["variant"], r["mode"]) for r in rows]
    run.require(len(keys) == len(expected) and set(keys) == expected, "missing/duplicate workload runs")
    outputs, allocations = {}, {}
    for row, key in zip(rows, keys):
        extent, engine, variant, mode = key
        folder = report_path.parent / "x".join(str(v) for v in extent) / f"{engine}-{variant}-{mode}"
        stdout, stderr = (folder / "stdout.txt").read_text(), (folder / "stderr.txt").read_text()
        run.require("Validation Error:" not in stdout + stderr, "validation error in retained log")
        run.require(bench.records(stdout, "MEASUREMENT ") == [row["metadata"]], "metadata/log mismatch")
        bench.check_metadata(row["metadata"], extent, mode, True)
        run.require(run_native.allocation_evidence(stdout, stderr, row["metadata"], engine) == row["memory"],
                    "allocation evidence/log mismatch")
        run.require(set(row["output_sha256"]) == set(bench.output_files()), "incomplete output hashes")
        for name, digest in row["output_sha256"].items():
            run.verified(folder / name, digest)
        if extent in outputs:
            run.require(outputs[extent] == row["output_sha256"], "output equality mismatch")
        else:
            outputs[extent] = row["output_sha256"]
        memory_key = (extent, mode)
        signature = row["memory"]["signature"]
        if memory_key in allocations:
            run.require(allocations[memory_key] == signature, "allocation-policy mismatch")
        else:
            allocations[memory_key] = signature
    report["input_report_sha256"] = run.digest_file(report_path)
    report["export_scope"] = "Full local output hashes and trace consistency rechecked; CPU oracle not rerun by export"
    # Exclusive creation: never silently replace another acceptance receipt.
    with destination.open("x") as output:
        output.write(json.dumps(report, indent=2, allow_nan=False) + "\n")
    print(f"Exported {len(rows)} validated workload records with output hashes and full allocation traces: {destination}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    export(args.report, args.destination)
