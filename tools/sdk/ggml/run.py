#!/usr/bin/env python3
"""Independent foundation GGML acceptance; no downloads, training or Rust build."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import resource
import signal
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent


def require(value,message):
    if not value: raise ValueError(message)


def digest(path):
    with path.open("rb") as file: return hashlib.file_digest(file,"sha256").hexdigest()


def verify_data(data,checksums):
    rows = [line.split() for line in checksums.splitlines()]
    for name in ("t10k-images-idx3-ubyte","t10k-labels-idx1-ubyte"):
        expected = [fields[0] for fields in rows if len(fields) == 2 and fields[1] == name]
        require(len(expected) == 1 and digest(data / name) == expected[0],"missing/duplicate/mismatched dataset digest")


def verify_transformer(text,placement):
    shapes=[(1,1,1),(13,29,3),(13,29,17),(64,128,16),(65,129,17)]
    def rows(prefix):return [dict(field.split('=',1) for field in line.split()[1:] if '=' in field)
                             for line in text.splitlines() if line.startswith(prefix+' ')]
    cases=rows('transformer')
    expected=[(str(d),str(h),str(t),p) for d,h,t in shapes for p in ('f32','f16')]
    require([(r.get('d'),r.get('h'),r.get('t'),r.get('precision')) for r in cases]==expected,'missing/duplicate transformer shape')
    require(sum(line.startswith('transformer ') and line.endswith(' PASS') for line in text.splitlines())==10,'missing transformer pass')
    require(all(r.get('phases')=='3' and r.get('dispatches')=='45' and r.get('rejected')=='11' and r.get('retry')=='15'
                and int(r['allocation_bytes'])>0 for r in cases),'incomplete transformer execution/rejection gates')
    errors=rows('transformer_error')
    names={'N','Q','K','V','scores','P','Vt','A','O','R','NR','up','relu','down','Y'}
    require(len(errors)==150 and {(r.get('d'),r.get('h'),r.get('t'),r.get('precision'),r.get('tensor')) for r in errors}==
            {(*case,name) for case in expected for name in names},'missing/duplicate transformer intermediate')
    require(all(math.isfinite(float(r['max_abs'])) and float(r['max_abs'])>=0 for r in errors),'nonfinite transformer error')
    drift=rows('transformer_weight_drift')
    require([(r.get('d'),r.get('h'),r.get('t')) for r in drift]==[tuple(map(str,s)) for s in shapes]
            and all(math.isfinite(float(r['max_abs'])) and float(r['max_abs'])>=0 for r in drift),'missing/invalid weight drift')
    require(text.count(f'Transformer profile placement={placement} cases=10 PASS;')==1,'missing transformer completion')
    return dict(cases=cases,errors=errors,weight_drift=drift)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data-dir",type=Path,help="optional already downloaded/extracted MNIST test data")
    args = parser.parse_args()
    folder = Path(tempfile.mkdtemp(prefix="run-",dir=HERE))
    print(f"Independent GGML artifacts: {folder}",flush=True)
    report = dict(schema=1,complete=False,full_dataset=args.data_dir is not None,runs=[],
                  scope="Foundation GGML correctness and lifetime handoff, not performance or Metal acceptance")
    def save(): (folder / "report.json").write_text(json.dumps(report,indent=2)+"\n")
    save()
    try:
        # Lifecycle death tests also disable Linux dumpability before violating
        # ownership. The parent prevents ordinary core-file generation as well.
        resource.setrlimit(resource.RLIMIT_CORE,(0,0))
        pkg = os.getenv("PKG_CONFIG","pkg-config")
        def variable(name): return subprocess.check_output([pkg,"--variable="+name,"ogpu-foundation"],text=True).strip()
        require(variable("ogpu_foundation") == "1","foundation revision mismatch")
        report["sdk"] = dict(revision=variable("ogpu_revision"),runtime_sha256=digest(Path(variable("libdir")) / "libogpu.so"),
                             header_sha256=digest(Path(variable("includedir")) / "ogpu_next.h"))
        report["sha256"] = {str(p.relative_to(HERE)):digest(p) for p in sorted(HERE.rglob("*"))
                            if p.is_file() and (p.parent == HERE and p.suffix in (".cpp",".h",".py") or p.parent.name == "shaders")}
        def execute(label,name,*arguments,expected=0):
            with (folder / (label+".stdout")).open("w") as out,(folder / (label+".stderr")).open("w") as err:
                p = subprocess.run([str(HERE / "build" / name),*map(str,arguments)],stdout=out,stderr=err,timeout=300)
            text = (folder / (label+".stdout")).read_text()+(folder / (label+".stderr")).read_text()
            require(p.returncode == expected,f"{label} returned {p.returncode}, expected {expected}")
            require(not any(s in text for s in ("Validation Error:","SYNC-HAZARD","runtime error:","ERROR: AddressSanitizer")),f"{label} validation/sanitizer error")
            return text
        for path in (HERE / "build").glob("ogpu-*"):
            if path.is_file(): report["sha256"][str(path.relative_to(HERE))] = digest(path)
        model = HERE / "fixtures/mnist-fc-f32.gguf"
        require(digest(model) == (HERE / "fixtures/SHA256SUMS").read_text().split()[0],"model digest mismatch")
        for relative in ("CMakeLists.txt","dataset.sha256","fixtures/SHA256SUMS","fixtures/mnist-fc-f32.gguf"):
            report["sha256"][relative] = digest(HERE / relative)
        require("recovery" in execute("recovery","ogpu-ggml-foundation-check"),"missing recording recovery gate")
        for placement in ("host","device"):
            text = execute(placement+"-lifecycle","ogpu-ggml-lifecycle",HERE / "shaders","normal",placement)
            require("scoped lifecycle PASS" in text,"missing lifecycle gate")
            for violation in ("live-backend","live-buffer"):
                text = execute(placement+"-"+violation,"ogpu-ggml-lifecycle",HERE / "shaders",violation,placement,expected=-signal.SIGABRT)
                require("OGPU session destroyed with live backends/buffers" in text,"missing lifetime diagnostic")
            text = execute(placement+"-matrix","ogpu-ggml-matrix-check",HERE / "shaders","0",placement)
            require(sum(line.startswith("mixed_matrix ") and line.endswith(" PASS") for line in text.splitlines()) == 2,"missing matrix gates")
            text = execute(placement+"-transformer","ogpu-ggml-transformer-check",HERE / "shaders","0",placement)
            transformer=verify_transformer(text,placement)
            report["runs"].append(dict(placement=placement,lifecycle=True,matrix_cases=2,transformer=transformer)); save()
        if args.data_dir:
            data = args.data_dir.resolve()
            verify_data(data,(HERE / "dataset.sha256").read_text())
            half,wide = folder / "half.gguf",folder / "widened.gguf"
            execute("convert","ogpu-ggml-convert",model,half,wide)
            report["weights"] = dict(original=digest(model),half=digest(half),wide=digest(wide))
            for precision in ("f32","f16"):
                extra = [half,wide] if precision == "f16" else []
                text = execute("mnist-"+precision,"ogpu-mnist",model,data / "t10k-images-idx3-ubyte",data / "t10k-labels-idx1-ubyte",
                               HERE / "shaders","0","device",*extra)
                gates = [line for line in text.splitlines() if line.startswith("batch=") and line.endswith(" PASS")]
                require(len(gates) == 6,"missing full-dataset gates")
                report["runs"].append(dict(precision=precision,placement="device",full_dataset=gates)); save()
        report["complete"] = True; save()
        print("Independent foundation GGML handoff PASS"+("; transformer and both full-dataset precision modes" if args.data_dir else "; transformer/matrix/lifetime"))
    except Exception as error:
        report["error"] = str(error); save(); raise


if __name__ == "__main__": main()
