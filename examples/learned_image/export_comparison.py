#!/usr/bin/env python3
"""Export a completed paired comparison and checked validation evidence."""
import argparse
import csv
import json
from pathlib import Path
import sys
sys.dont_write_bytecode = True
import benchmark as bench
import compare_native
import export_native_workload
import run
import run_native


def export(report_path, destination):
    report = json.loads(report_path.read_text())
    run.require(report.get("complete") is True and report.get("schema") == 1, "incomplete comparison")
    expected = {(extent, engine, mode, index) for extent in bench.EXTENTS
                for engine in compare_native.ENGINES for mode in bench.MODES for index in range(3)}
    keys = [(tuple(r["extent"]), r["engine"], r["mode"], r["round"]) for r in report["runs"]]
    run.require(len(keys) == len(expected) and set(keys) == expected, "missing/duplicate timing controls")
    memory_keys = [(tuple(r["extent"]), r["engine"], r["mode"]) for r in report["memory"]]
    run.require(len(memory_keys) == 16 and set(memory_keys) == {k[:3] for k in expected}, "missing/duplicate memory controls")
    validation_path = Path(report["validation_report"])
    run.verified(validation_path, report["validation_sha256"])
    validated = json.loads(validation_path.read_text())
    run.require(report["revision"] == validated["revision"] and report["dirty"] == validated["dirty"], "validation provenance mismatch")
    finals = {tuple(r["extent"]): r["output_sha256"]["normal-1-final.rgba"] for r in validated["runs"]}
    samples = []
    for row in report["runs"]:
        extent, mode, engine = row["extent"], row["mode"], row["engine"]
        folder = report_path.parent / "x".join(str(v) for v in extent) / f"run-{row['round']}-{engine}-{mode}"
        stdout = (folder / "stdout.txt").read_text()
        metadata, checked = compare_native.checked_samples(stdout, extent, mode, report["identity"])
        run.require(metadata == row["metadata"] and checked == row["samples"]
                    and bench.summarize(checked) == row["statistics"], "timing log/report mismatch")
        run.require(row["final_sha256"] == finals[tuple(extent)], "timed output/validation mismatch")
        run.verified(folder / "normal-39-final.rgba", row["final_sha256"])
        for sample in checked:
            samples.append(dict(width=extent[0], height=extent[1], out_width=extent[2], out_height=extent[3],
                                engine=engine, mode=mode, round=row["round"], **sample))
    signatures = {}
    for row in report["memory"]:
        extent, engine, mode = row["extent"], row["engine"], row["mode"]
        folder = report_path.parent / "x".join(str(v) for v in extent) / f"memory-{engine}-{mode}"
        stdout, stderr = (folder / "stdout.txt").read_text(), (folder / "stderr.txt").read_text()
        metadata, _ = compare_native.checked_samples(stdout, extent, mode, report["identity"])
        allocation = run_native.allocation_evidence(stdout, stderr, metadata, engine)
        run.require(metadata == row["requested"] and allocation == row["allocation"], "memory trace/report mismatch")
        key = (tuple(extent), mode)
        if key in signatures:
            run.require(signatures[key] == allocation["signature"], "native/OGPU allocation mismatch")
        signatures[key] = allocation["signature"]
        run.require(row["final_sha256"] == finals[tuple(extent)], "traced output/validation mismatch")
        run.verified(folder / "normal-39-final.rgba", row["final_sha256"])
    destination.mkdir(exist_ok=False)
    export_native_workload.export(validation_path, destination / "validation.json")
    for row in report["runs"]:
        del row["samples"]
    report["input_report_sha256"] = run.digest_file(report_path)
    report["raw_samples"] = "samples.csv"
    report["validation_report"] = "validation.json"
    report["exported_validation_sha256"] = run.digest_file(destination / "validation.json")
    with (destination / "samples.csv").open("w", newline="") as output:
        writer = csv.DictWriter(output, fieldnames=list(samples[0]), lineterminator="\n")
        writer.writeheader(); writer.writerows(samples)
    (destination / "report.json").write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    print(f"Exported {len(samples)} paired samples, 16 allocation traces and full correctness evidence: {destination}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    export(args.report, args.destination)
