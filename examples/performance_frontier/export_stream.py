#!/usr/bin/env python3
"""Export complete streaming frontier evidence after rechecking retained logs."""
import argparse
import csv
import importlib.util
import json
from pathlib import Path
import sys
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("frontier_stream", Path(__file__).with_name("stream.py"))
stream = importlib.util.module_from_spec(spec); spec.loader.exec_module(stream)
f = stream.f


def export(source, destination):
    report = json.loads(source.read_text())
    f.require(report.get("schema") == 1 and report.get("complete") is True and not report.get("preflight"), "incomplete/preflight report")
    extents = ((65,47,131,95),) if report["software"] else ((1280,720,2560,1440),(1919,1079,2561,1441))
    f.require(not report.get("compiled_only") or report["software"], "compiled matrix is correctness only")
    policies = ("compiled",) if report.get("compiled_only") else f.POLICIES
    expected = {(e, s, p) for e in extents for s in (1,2,3) for p in policies}
    all_samples = []; signatures = {}
    for section in ("validation", "timing", "memory"):
        rows = report[section]
        wanted = set() if report["software"] and section != "validation" else (
            {(*key, r) for key in expected for r in range(3)} if section == "timing" else expected)
        keys = [(tuple(row["result"]["extent"]), row["result"]["slots"], row["result"]["policy"],
                 *([row["round"]] if section == "timing" else [])) for row in rows]
        f.require(len(keys) == len(wanted) and set(keys) == wanted, f"incomplete/duplicate {section} matrix")
        for row in rows:
            folder = source.parent / row["label"]; r = row["result"]
            for name in ("stdout", "stderr"):
                f.require(f.digest(folder / f"{name}.txt") == row[f"{name}_sha256"], "log hash mismatch")
            stdout = (folder / "stdout.txt").read_text(); stderr = (folder / "stderr.txt").read_text()
            f.require("Validation Error:" not in stdout + stderr, "validation error")
            result, device, samples = stream.parse(stdout, r["policy"], r["slots"], tuple(r["extent"]),
                                                   64 if report["software"] else 1000, section == "validation")
            f.require(result == r and device == report["identity"], "result/identity mismatch")
            if section == "timing":
                f.require(samples == row["samples"] and stream.summarize(samples) == row["statistics"], "sample/statistics mismatch")
                for sample in samples:
                    all_samples.append(dict(extent="x".join(map(str, r["extent"])), slots=r["slots"], policy=r["policy"], round=row["round"], **sample))
            else:
                memory = f.bench.parse_memory(stderr)
                f.require(memory == row["memory"] and memory["allocations"] == memory["frees"] == memory["peak_count"] == 1 + 8 * r["slots"], "memory mismatch")
                trace = [line for line in stderr.splitlines() if line.startswith(("ALLOCATE ", "FREE ", "MEMORY_SUMMARY "))]
                f.require(trace == row["trace"], "trace mismatch")
                sig = [(a["bytes"], a["type"]) for a in f.bench.records(stderr, "ALLOCATE ")]
                key = (tuple(r["extent"]), r["slots"])
                f.require(sig == signatures.setdefault(key, sig), "allocation policy mismatch")
            row.pop("samples", None)
    for name, digest in report["sources"].items():
        f.require(f.digest(f.ROOT / name) == digest, f"source mismatch: {name}")
    for files in report["fixtures"].values():
        for path, digest in files.items():
            f.require(f.digest(Path(path)) == digest, "fixture hash mismatch")
    destination.mkdir(exist_ok=False)
    report["input_report_sha256"] = f.digest(source)
    if all_samples:
        with (destination / "samples.csv").open("w", newline="") as output:
            writer = csv.DictWriter(output, fieldnames=list(all_samples[0]), lineterminator="\n")
            writer.writeheader(); writer.writerows(all_samples)
        report["raw_samples"] = "samples.csv"; report["raw_samples_sha256"] = f.digest(destination / "samples.csv")
    report["export_scope"] = "log hashes, matrix, statistics, source/fixture hashes and full allocation traces rechecked; GPU oracle not rerun"
    (destination / "report.json").write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    print(f"Exported streaming evidence: {destination}; {len(all_samples)} samples")


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__); p.add_argument("source", type=Path); p.add_argument("destination", type=Path)
    args = p.parse_args(); export(args.source, args.destination)
