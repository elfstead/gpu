#!/usr/bin/env python3
"""Recheck retained frontier logs/traces and export reviewable evidence."""
import argparse
import csv
import importlib.util
import json
from pathlib import Path
import sys
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("frontier", Path(__file__).with_name("run.py"))
frontier = importlib.util.module_from_spec(spec)
spec.loader.exec_module(frontier)


def export(source, destination):
    report = json.loads(source.read_text())
    frontier.require(report.get("complete") is True and report.get("schema") == 1, "incomplete report")
    expected = {(slots, dispatches, policy) for slots in (1, 3) for dispatches in (1, 64) for policy in frontier.POLICIES}
    samples = []
    signatures = {}
    for section in ("validation", "timing", "memory"):
        rows = report[section]
        wanted = set() if report["software"] and section != "validation" else (
            {(s, d, p, r) for s, d, p in expected for r in range(3)} if section == "timing" else expected)
        keys = [(row["result"]["slots"], row["result"]["dispatches"], row["result"]["policy"], *([row["round"]] if section == "timing" else [])) for row in rows]
        frontier.require(len(keys) == len(wanted) and set(keys) == wanted, f"incomplete/duplicate {section} matrix")
        for row in rows:
            r = row["result"]; folder = source.parent / row["label"]
            for name in ("stdout", "stderr"):
                frontier.require(frontier.digest(folder / f"{name}.txt") == row[f"{name}_sha256"], "log hash mismatch")
            stdout = (folder / "stdout.txt").read_text(); stderr = (folder / "stderr.txt").read_text()
            frontier.require("Validation Error:" not in stdout + stderr, "retained validation error")
            count = 64 if report["software"] else 1000
            result, device, frames = frontier.parse(stdout, r["policy"], r["slots"], r["dispatches"], count, section == "validation")
            frontier.require(result == r and device == report["identity"] == row["device"], "result/identity mismatch")
            if section == "timing":
                frontier.require(frames == row["frames"] and frontier.summary(frames) == row["statistics"], "sample/statistics mismatch")
                for frame in frames:
                    samples.append(dict(policy=r["policy"], slots=r["slots"], dispatches=r["dispatches"], round=row["round"], **frame))
            else:
                memory = frontier.bench.parse_memory(stderr)
                frontier.require(memory == row["memory"] and memory["allocations"] == memory["frees"] == memory["peak_count"] == r["slots"], "allocation evidence mismatch")
                signature = [(a["bytes"], a["type"]) for a in frontier.bench.records(stderr, "ALLOCATE ")]
                frontier.require(signature == signatures.setdefault(r["slots"], signature), "memory policy mismatch")
                trace = [line for line in stderr.splitlines() if line.startswith(("ALLOCATE ", "FREE ", "MEMORY_SUMMARY "))]
                frontier.require(trace == row["trace"], "trace mismatch")
            row.pop("frames", None)
    for name, digest in report["sources"].items():
        frontier.require(frontier.digest(frontier.ROOT / name) == digest, f"source changed since run: {name}")
    destination.mkdir(exist_ok=False)
    report["input_report_sha256"] = frontier.digest(source)
    if samples:
        with (destination / "samples.csv").open("w", newline="") as output:
            writer = csv.DictWriter(output, fieldnames=list(samples[0]), lineterminator="\n")
            writer.writeheader(); writer.writerows(samples)
        report["raw_samples"] = "samples.csv"
        report["raw_samples_sha256"] = frontier.digest(destination / "samples.csv")
    report["export_scope"] = "retained log hashes, matrix, sample statistics and allocation traces rechecked; GPU oracle not rerun"
    (destination / "report.json").write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    print(f"Exported checked frontier evidence ({len(samples)} raw samples): {destination}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path); parser.add_argument("destination", type=Path)
    args = parser.parse_args(); export(args.source, args.destination)
