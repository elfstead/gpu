#!/usr/bin/env python3
"""Independent foundation GGML acceptance; no downloads, training or Rust build."""
import argparse
import hashlib
import json
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
            require(not any(s in text for s in ("Validation Error:","runtime error:","ERROR: AddressSanitizer")),f"{label} validation/sanitizer error")
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
            report["runs"].append(dict(placement=placement,lifecycle=True,matrix_cases=2)); save()
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
        print("Independent foundation GGML handoff PASS"+("; both full-dataset precision modes" if args.data_dir else "; matrix/lifetime only"))
    except Exception as error:
        report["error"] = str(error); save(); raise


if __name__ == "__main__": main()
