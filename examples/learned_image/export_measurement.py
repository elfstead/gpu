#!/usr/bin/env python3
"""Export a completed local report and raw samples to a new, reviewable directory."""
import argparse
import csv
import json
from pathlib import Path
import sys
sys.dont_write_bytecode = True
import benchmark as bench
import run


def export(report_path, destination):
    report = json.loads(report_path.read_text())
    run.require(report.get("complete") is True and report.get("schema") == 1, "report is not complete")
    runs = report["runs"]
    expected = {(extent, mode, round_index) for extent in bench.EXTENTS for mode in bench.MODES for round_index in range(3)}
    keys = [(tuple(row["extent"]), row["mode"], row["round"]) for row in runs]
    run.require(len(keys) == len(expected) and set(keys) == expected, "missing/duplicate timing runs")
    samples = []
    for row in runs:
        text = "MEASUREMENT " + json.dumps(row["metadata"]) + "\n"
        text += "\n".join("SAMPLE " + json.dumps(sample) for sample in row["samples"])
        _, checked = bench.parse_samples(text, row["mode"])
        bench.check_metadata(row["metadata"], row["extent"], row["mode"], False)
        run.require(bench.summarize(checked) == row["statistics"], "stale summary statistics")
        for sample in checked:
            samples.append(dict(width=row["extent"][0], height=row["extent"][1], out_width=row["extent"][2],
                                out_height=row["extent"][3], mode=row["mode"], round=row["round"], **sample))
    memory_keys = [(tuple(row["extent"]), row["mode"]) for row in report["memory"]]
    run.require(len(memory_keys) == 8 and set(memory_keys) == {(e, m) for e in bench.EXTENTS for m in bench.MODES},
                "missing/duplicate memory runs")
    for row in report["memory"]:
        label = "x".join(str(v) for v in row["extent"])
        path = report_path.parent / label / f"memory-{row['mode']}" / "stderr.txt"
        text = path.read_text()
        run.require(bench.parse_memory(text) == row["native"], "memory summary/trace disagreement")
        row["trace"] = [line for line in text.splitlines() if line.startswith(("ALLOCATE ", "FREE ", "MEMORY_SUMMARY "))]
    report["input_report_sha256"] = run.digest_file(report_path)
    report["raw_samples"] = "samples.csv"
    for row in runs:
        del row["samples"]
    # Refuse overwrite; a receipt export must not quietly replace earlier evidence.
    destination.mkdir(parents=False, exist_ok=False)
    with (destination / "samples.csv").open("w", newline="") as output:
        writer = csv.DictWriter(output, fieldnames=list(samples[0]))
        writer.writeheader()
        writer.writerows(samples)
    (destination / "report.json").write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    print(f"Exported {len(samples)} samples, per-run distributions and eight complete native allocation traces: {destination}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    export(args.report, args.destination)
