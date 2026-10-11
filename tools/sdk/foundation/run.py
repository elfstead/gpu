#!/usr/bin/env python3
"""Standalone learned-image foundation acceptance; no checkout/compiler imports."""
import hashlib
import json
import math
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
sys.dont_write_bytecode = True
import reference as ref

HERE = Path(__file__).resolve().parent
EXTENT = (65,47,131,95)


def require(value,message):
    if not value: raise ValueError(message)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check_floats(path,expected):
    data = path.read_bytes()
    require(len(data) == 4*len(expected),"wrong scalar count")
    maximum = ratio = 0.
    for (value,),correct in zip(struct.iter_unpack("<f",data),expected):
        bound = 2e-5+2e-5*abs(correct)
        require(math.isfinite(value) and abs(value-correct) <= bound,"scalar oracle mismatch")
        maximum,ratio = max(maximum,abs(value-correct)),max(ratio,abs(value-correct)/bound)
    return maximum,ratio


def check_pixels(path,expected):
    data = path.read_bytes()
    require(len(data) == len(expected) and len(data)%4 == 0,"wrong pixel count")
    delta = 0
    for i,(actual,correct) in enumerate(zip(data,expected)):
        if i%4 == 3: require(actual == correct == 255,"alpha mismatch")
        else:
            delta = max(delta,abs(actual-correct))
            require(delta <= 1,"pixel oracle mismatch")
    return delta


def main():
    destination = Path(tempfile.mkdtemp(prefix="run-",dir=HERE))
    print(f"Foundation handoff artifacts: {destination}",flush=True)
    report = dict(schema=1,complete=False,extent=EXTENT,runs=[],
                  scope="Serial resident/end-to-end A/B/A correctness, not performance or Metal acceptance")
    def save(): (destination / "report.json").write_text(json.dumps(report,indent=2,allow_nan=False)+"\n")
    save()
    try:
        pkg = os.getenv("PKG_CONFIG","pkg-config")
        revision = subprocess.check_output([pkg,"--variable=ogpu_revision","ogpu-foundation"],text=True).strip()
        libdir = Path(subprocess.check_output([pkg,"--variable=libdir","ogpu-foundation"],text=True).strip())
        report["sdk"] = dict(revision=revision,runtime_sha256=digest(libdir / "libogpu.so"))
        report["sha256"] = {p.name:digest(p) for p in sorted(HERE.iterdir()) if p.is_file() and p.suffix in (".c",".h",".py",".json")}
        report["sha256"]["consumer"] = digest(HERE / "consumer")
        model = json.loads((HERE / "model.json").read_text())
        weights = model["weights"]; ref.check_weights(weights)
        packed = ref.packed(weights)
        require(hashlib.sha256(packed).hexdigest() == model["weight_sha256"],"model weights changed")
        (destination / "weights.f32").write_bytes(packed)
        oracles = []
        for label,seed in (("a",2001),("b",2002)):
            case = destination / label; case.mkdir()
            _,noisy = ref.scene(seed,*EXTENT[:2])
            hidden,denoised = ref.infer(weights,noisy,*EXTENT[:2])
            processed,pixels = ref.render_reference(denoised,*EXTENT)
            (case / "input.f32").write_bytes(ref.packed(noisy))
            oracles.append(dict(input=ref.packed(noisy),weights=packed,hidden=hidden,denoised=denoised,processed=processed,pixels=bytes(pixels)))
        save()
        baseline = None
        for mode in ("resident","end-to-end"):
            output = destination / mode; output.mkdir()
            command = [str(HERE / "consumer"),"--validate",mode,str(destination / "weights.f32"),
                       *map(str,EXTENT),str(output),str(destination / "a"),str(destination / "b")]
            with (output / "stdout.txt").open("w") as stdout,(output / "stderr.txt").open("w") as stderr:
                process = subprocess.run(command,stdout=stdout,stderr=stderr,timeout=120)
            text = (output / "stdout.txt").read_text()+(output / "stderr.txt").read_text()
            require(process.returncode == 0 and "Validation Error:" not in text,f"{mode} execution failed; inspect logs")
            require(sum("output guards, no intermediate CPU read/wait PASS" in line for line in text.splitlines()) == 6,"missing frame/guard checks")
            records = [json.loads(line[len("REPLAY "):]) for line in text.splitlines() if line.startswith("REPLAY ")]
            require(len(records) == 1,"missing/duplicate replay policy")
            policy = dict(mode=mode,validation=True,warmups=0,frames=3,work_lists=2 if mode == "resident" else 1,
                          upload_bytes=0 if mode == "resident" else EXTENT[0]*EXTENT[1]*4+128,
                          readback_bytes=0 if mode == "resident" else EXTENT[2]*EXTENT[3]*4)
            require(all(type(records[0].get(k)) is type(v) and records[0][k] == v for k,v in policy.items()),"wrong replay policy")
            maximum = ratio = 0.; delta = 0
            for frame in range(3):
                oracle = oracles[frame%2]
                normal = output / f"normal-{frame}-final.rgba"
                require(normal.read_bytes() == (output / f"diagnostic-{frame}-final.rgba").read_bytes(),"diagnostic changed output")
                delta = max(delta,check_pixels(normal,oracle["pixels"]))
                for stage in ("input","weights"):
                    require((output / f"diagnostic-{frame}-{stage}.f32").read_bytes() == oracle[stage],f"changed {stage}")
                for stage in ("hidden","denoised","processed"):
                    error,bound = check_floats(output / f"diagnostic-{frame}-{stage}.f32",oracle[stage])
                    maximum,ratio = max(maximum,error),max(ratio,bound)
            files = sorted([*output.glob("*.rgba"),*output.glob("*.f32")])
            require(len(files) == 21,"unexpected output set")
            for path in files:
                if "-2-" in path.name: require(path.read_bytes() == path.with_name(path.name.replace("-2-","-0-")).read_bytes(),"A replay differs")
                if baseline: require(path.read_bytes() == (baseline / path.name).read_bytes(),"mode output differs")
            require((output / "normal-0-final.rgba").read_bytes() != (output / "normal-1-final.rgba").read_bytes(),"A/B not distinguished")
            baseline = output
            report["runs"].append(dict(mode=mode,frames=6,max_error=maximum,max_ratio=ratio,pixel_delta=delta,policy=records[0]))
            save()
        report["complete"] = True; save()
        print("Foundation handoff PASS: 12 checked frames, frozen scalar/pixel gates, unchanged inputs/weights and A/B/A replay")
    except Exception as error:
        report["error"] = str(error); save(); raise


if __name__ == "__main__": main()
