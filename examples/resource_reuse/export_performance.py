#!/usr/bin/env python3
"""Revalidate complete raw logs and export the matched range comparison with all timing samples."""
import argparse
import csv
import importlib.util
import json
from pathlib import Path
import sys
sys.dont_write_bytecode = True
spec=importlib.util.spec_from_file_location("reuse_performance",Path(__file__).with_name("performance.py"))
p=importlib.util.module_from_spec(spec); spec.loader.exec_module(p)
require,f=p.require,p.f


def export(source,destination):
    report=json.loads(source.read_text())
    require(report.get("schema")==1 and report.get("complete") is True and report.get("dirty") is False,"incomplete/dirty report")
    extents=p.EXTENTS[:1] if report["software"] else p.EXTENTS
    expected={(e,s,a,policy) for e in extents for s in (1,2,3) for a,policy in p.CONTROLS}
    all_samples=[]; signatures={}; layouts={}; budgets={}
    for section in ("serial","validation","timing","memory"):
        rows=report[section]
        def key(row):
            r=row["result"]
            return (tuple(r["extent"]),r["slots"],row["backing"]["allocation"],r["policy"],
                    *([row["round"]] if section=="timing" else []))
        wanted=({(e,1,"dedicated","ogpu") for e in extents} if section=="serial" else
                set() if report["software"] and section in ("timing","memory") else
                {(*k,r) for k in expected for r in range(p.ROUNDS)} if section=="timing" else expected)
        require(len(rows)==len(wanted) and {key(row) for row in rows}==wanted,"incomplete/duplicate "+section)
        for row in rows:
            r=row["result"]; allocation=row["backing"]["allocation"]; extent=tuple(r["extent"])
            files={suffix:source.parent/(row["label"]+'.'+suffix) for suffix in ("stdout","stderr")}
            for suffix,path in files.items(): require(f.digest(path)==row[suffix+"_sha256"],"log mismatch")
            count=12 if section=="serial" else p.FRAMES if section=="timing" else p.VALIDATE
            checked,samples=p.parse(files["stdout"].read_text(),files["stderr"].read_text(),allocation,r["policy"],r["slots"],
                                    extent,count,section in ("serial","validation"),section!="timing")
            for name,value in checked.items():
                require(json.loads(json.dumps(value))==row[name],f"reparsed {name} mismatch")
            require(row["device"]==report["identity"],"device mismatch")
            layout_key=(extent,r["slots"],allocation)
            require(row["ranges"]==layouts.setdefault(layout_key,row["ranges"]),"layout mismatch")
            require(row["backing"]["buffer_requested_bytes"]==budgets.setdefault((extent,r["slots"]),row["backing"]["buffer_requested_bytes"]),"budget mismatch")
            if "allocation_signature" in row:
                require(row["allocation_signature"]==signatures.setdefault(layout_key,row["allocation_signature"]),"allocation mismatch")
            if samples:
                path=source.parent/row["samples_file"]
                require(f.digest(path)==row["samples_sha256"],"sample hash mismatch")
                with path.open() as input_file:
                    saved=[{k:int(v) if k=="index" else float(v) for k,v in sample.items()} for sample in csv.DictReader(input_file)]
                require(saved==samples,"samples differ from raw log")
                if section=="timing":
                    all_samples.extend(dict(label=row["label"],**sample) for sample in samples)
            row.pop("samples_file",None); row.pop("samples_sha256",None)
    if not report["software"]:
        require(p.comparisons(report["timing"])==report["comparisons"],"comparison summary mismatch")
        sequence=[(r,e,s,a,policy) for r in range(p.ROUNDS) for ei,e in enumerate(extents)
                  for s in (1,2,3) for a,policy in p.order(r,ei,s)]
        require([(v["round"],tuple(v["result"]["extent"]),v["result"]["slots"],v["backing"]["allocation"],v["result"]["policy"])
                 for v in report["timing"]]==sequence,"process order mismatch")
        timed=report["environments"]["timing"]
        require(timed["VK_LOADER_LAYERS_DISABLE"]=="*" and not timed["VK_INSTANCE_LAYERS"]
                and not timed["VK_LAYER_VALIDATE_SYNC"] and not timed["OGPU_TRACE_LOADER"],"instrumented timing environment")
    for name,digest in report["sources"].items(): require(f.digest(p.ROOT/name)==digest,"source mismatch: "+name)
    for fixture in report["fixtures"].values():
        for name,digest in fixture.items(): require(f.digest(Path(name))==digest,"fixture mismatch")
    destination.mkdir(exist_ok=False)
    if all_samples:
        with (destination/"samples.csv").open("w",newline="") as output:
            writer=csv.DictWriter(output,fieldnames=list(all_samples[0]),lineterminator="\n")
            writer.writeheader(); writer.writerows(all_samples)
        report["raw_samples_sha256"]=f.digest(destination/"samples.csv")
        report["raw_samples"]="samples.csv"
    report["input_report_sha256"]=f.digest(source)
    report["export_scope"]="raw logs, full matrix/order, statistics, allocation/range matching, source/fixture hashes revalidated; GPU oracle not rerun"
    (destination/"report.json").write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
    print(f"Exported {destination}: {len(all_samples)} timing samples")


if __name__=="__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source",type=Path); parser.add_argument("destination",type=Path)
    args=parser.parse_args(); export(args.source,args.destination)
