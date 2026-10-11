#!/usr/bin/env python3
"""Independent foundation libplacebo acceptance; no downloads or OGPU build."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
EXECUTABLES = {"consumer","backend-tests","stream","perf","hdr","reference","compare-hdr","compare-reference"}


def require(value,message):
    if not value: raise ValueError(message)


def digest(path):
    with path.open("rb") as file: return hashlib.file_digest(file,"sha256").hexdigest()


def verify_build(record,sdk):
    require(record["schema"] == 1 and record["complete"] and
            record["upstream_revision"] == "3330a515d62139259c26239014f286e233bd3a5c","invalid build receipt")
    require(record["sdk"] == sdk,"SDK changed since build")
    sources = {p.name for p in HERE.iterdir() if p.suffix in (".c",".cpp",".h",".hpp",".py")}
    require(set(record["sources_sha256"]) == sources,"incomplete consumer source receipt")
    require(set(record["executables_sha256"]) == EXECUTABLES,"incomplete executable receipt")
    build = Path(record["build"])
    for name,expected in record["sources_sha256"].items(): require(digest(HERE / name) == expected,"consumer source changed since build")
    for name,expected in record["executables_sha256"].items(): require(digest(build / name) == expected,"consumer executable changed since build")
    require(digest(build / "upstream/src/libplacebo.so") == record["upstream_library_sha256"],"upstream library changed since build")
    return build


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--large",action="store_true",help="also verify 960x540 and 1920x1080 input; no timing runs")
    args = parser.parse_args()
    folder = Path(tempfile.mkdtemp(prefix="run-",dir=HERE))
    print(f"Independent libplacebo artifacts: {folder}",flush=True)
    report = dict(schema=1,complete=False,runs=[],scope="Foundation SDR/HDR, frame lifetime and native-matched correctness; not performance or Metal")
    def save(): (folder / "report.json").write_text(json.dumps(report,indent=2)+"\n")
    save()
    try:
        resource.setrlimit(resource.RLIMIT_CORE,(0,0))
        record = json.loads((HERE / "build.json").read_text())
        pkg = os.getenv("PKG_CONFIG","pkg-config")
        def variable(name): return subprocess.check_output([pkg,"--variable="+name,"ogpu-foundation"],text=True).strip()
        require(variable("ogpu_foundation") == "1","foundation revision mismatch")
        report["sdk"] = dict(revision=variable("ogpu_revision"),runtime_sha256=digest(Path(variable("libdir")) / "libogpu.so"),
                             header_sha256=digest(Path(variable("includedir")) / "ogpu_next.h"))
        build = verify_build(record,report["sdk"])
        require(Path(os.environ.get("VK_DRIVER_FILES","/missing-icd")).is_file(),"select exactly one ICD with VK_DRIVER_FILES")
        report["build"] = record
        for name in ("consumer","backend-tests","stream","perf","hdr"):
            linked = subprocess.check_output(["ldd",str(build / name)],text=True)
            (folder / (name+".ldd")).write_text(linked)
            for library,path in (("libogpu.so",Path(variable("libdir")) / "libogpu.so"),
                                 ("libplacebo.so.371",build / "upstream/src/libplacebo.so.371")):
                selected = next((line.split("=>",1)[1].rsplit(" (",1)[0].strip() for line in linked.splitlines()
                                 if line.strip().startswith(library+" =>")),None)
                require(selected is not None and Path(selected).resolve() == path.resolve(),f"{name} resolved unexpected {library}")
        def execute(label,name,*arguments,expected=0):
            with (folder / (label+".stdout")).open("w") as out,(folder / (label+".stderr")).open("w") as err:
                process = subprocess.run([str(build / name),*map(str,arguments)],stdout=out,stderr=err,timeout=300)
            text = (folder / (label+".stdout")).read_text()+(folder / (label+".stderr")).read_text()
            require(process.returncode == expected,f"{label} returned {process.returncode}, expected {expected}")
            require(not any(s in text for s in ("Validation Error:","runtime error:","ERROR: AddressSanitizer")),f"{label} validation/sanitizer error")
            return text
        frame_modes = ("frame-reuse","frame-capacity","frame-partial","frame-staging","frame-destroy","frame-pending",
                       "frame-poll-error","frame-submit-error","queued-specialization","queued-bank-reuse","frame-end-error")
        for mode in ("unorm16-sampling","specialization-update","partial-upload","unsupported-image","unsupported-shader",*frame_modes):
            require("live=0 PASS" in execute(mode,"backend-tests",mode),"missing adapter gate")
        for mode in frame_modes:
            require("live=0 PASS" in execute(mode+"-batched","backend-tests",mode,"batched"),"missing batched adapter gate")
        for mode,diagnostic in (("live-child","live children at device destruction"),("wait-error","completion unavailable; refusing unsafe teardown")):
            require(diagnostic in execute(mode,"backend-tests",mode,expected=-signal.SIGABRT),"missing fail-stop diagnostic")
        report["runs"].append(dict(kind="lifetime",cases=29)); save()
        reference,consumer = folder / "reference",folder / "consumer"
        reference.mkdir(); consumer.mkdir()
        require("upstream-reference creates=6 compute=9 raster=9 PASS" in execute("reference","reference",reference),"missing native SDR gate")
        require("ogpu-consumer creates=6 compute=9 raster=9 uploads=12 downloads=18 live=0 PASS" in execute("consumer","consumer",consumer),"missing SDR gate")
        require("PASS" in execute("sdr-compare","compare-reference",reference,consumer,"--consumer"),"missing SDR comparison")
        for mode in ("sync","two"):
            stream = folder / ("stream-"+mode); stream.mkdir()
            text = execute("stream-"+mode,"stream",stream,mode)
            require(sum(line.startswith("stream=") and line.endswith(" PASS") for line in text.splitlines()) == 36,"missing stream frames")
            require("PASS" in execute("stream-compare-"+mode,"compare-reference",reference,stream,"--stream"),"missing streaming comparison")
        report["runs"].append(dict(kind="sdr",cases=9,stream_frames=72)); save()
        native,hdr = folder / "hdr-native",folder / "hdr"; native.mkdir(); hdr.mkdir()
        require("HDR-reference creates=6 compute=9 raster=9 alpha_failures=0 PASS (native)" in execute("hdr-native","hdr",native),"missing native HDR gate")
        text = execute("hdr","hdr",hdr,"ogpu")
        require("HDR-reference creates=6 compute=9 raster=9 alpha_failures=0 PASS (ogpu)" in text and "HDR-ogpu live=0 receipt_reuses=12 PASS" in text,"missing HDR gate")
        for i in range(18): require((native / f"run-{i}.push").read_bytes() == (hdr / f"run-{i}.push").read_bytes(),"changed HDR parameters")
        require("HDR-comparison cases=9 PASS" in execute("hdr-compare","compare-hdr",native,hdr),"missing HDR comparison")
        report["runs"].append(dict(kind="hdr",cases=9)); save()
        for w,h in ([(64,33),(960,540),(1920,1080)] if args.large else [(64,33)]):
            for mode in ("resident","transfers"):
                text = execute(f"verify-{w}-{h}-{mode}","perf","verify",mode,w,h,64)
                require(sum(line.startswith("VERIFY ") and line.endswith(" PASS") for line in text.splitlines()) == 3,"missing scale verification")
                report["runs"].append(dict(kind="scale",width=w,height=h,mode=mode,engines=3)); save()
        report["complete"] = True; save()
        print("Independent foundation libplacebo handoff PASS; correctness only")
    except Exception as error:
        report["error"] = str(error); save(); raise


if __name__ == "__main__": main()
