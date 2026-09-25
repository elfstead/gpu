"""Optional relocatable source/build receipts, not a cache or hermetic build service."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def snapshot(paths):
    result = {}
    for path in sorted(set(p.resolve() for p in paths)):
        require(path.suffix not in (".slang-module", ".slang-lib"), "precompiled modules are outside the build-input profile")
        require(path.is_file(), f"missing compiler input: {path}")
        result[str(path)] = digest(path)
    require(result, "compiler reported no source inputs")
    return result


def include_report(compiler, source, options, extra=()):
    # Pinned Slang does not emit -depfile under -no-codegen. Its include report
    # includes imports as well as nested includes, including reflection queries.
    result = subprocess.run([compiler, str(source), *options, *extra, "-no-codegen", "-output-includes"],
                            text=True, capture_output=True)
    require(result.returncode == 0, f"compiler dependency/layout scan failed:\n{result.stdout}{result.stderr}")
    paths = []
    for line in (result.stdout+result.stderr).splitlines():
        if line.startswith("note: include"):
            match = re.fullmatch(r"note: include\s+'(.+)'", line)
            require(match is not None, "unsupported compiler include report")
            paths.append(Path(match[1]).resolve())
    require(paths and source.resolve() in paths, "compiler include report omitted entry source")
    return set(paths)


def depfile(path):
    """Read one compiler-emitted Make rule, including escaped spaces/#/$/colon.

    Not a shell parser. Reject variables, extra rules and ambiguous escapes rather
    than silently inventing dependencies. Slang emits one absolute-path rule.
    """
    text = path.read_text().replace("\\\n", "")
    require("\n" not in text.strip(), "multiple depfile rules/lines unsupported")
    words, word, separated, target = [], [], False, []
    index = 0
    while index < len(text):
        char = text[index]
        if char == "\\":
            index += 1
            require(index < len(text) and text[index] in " \\#:$\t", "unsupported depfile escape")
            word.append(text[index])
        elif char == "$":
            require(index+1 < len(text) and text[index+1] == "$", "depfile variables unsupported")
            word.append("$"); index += 1
        elif char == ":":
            require(not separated and word and not words, "unsupported depfile rule/colon")
            target = ["".join(word)]; word = []; separated = True
        elif char.isspace():
            if word: words.append("".join(word)); word = []
        else:
            require(char not in "#;|", "unsupported depfile syntax")
            word.append(char)
        index += 1
    if word: words.append("".join(word))
    require(separated and target and words, "missing depfile target/inputs")
    return {Path(word).resolve() for word in words}


def add_options(parser):
    parser.add_argument("--manifest", type=Path, help="optional checked build-input/output JSON receipt")
    parser.add_argument("--source-root", type=Path, help="declared source tree root, required with --manifest")
    parser.add_argument("--include-dir", action="append", default=[], type=Path, help="ordered Slang include/import search path")


def validate_options(args):
    require(bool(args.manifest) == bool(args.source_root), "--manifest and --source-root must be supplied together")
    if args.source_root: require(args.source_root.is_dir(), "source root is not a directory")
    for directory in args.include_dir: require(directory.is_dir(), f"include directory does not exist: {directory}")


def manifest_content(root, manifest, output, content, compilations):
    root = root.resolve()

    def relative(path):
        try: return str(Path(path).resolve().relative_to(root))
        except ValueError: raise ValueError(f"compiler input/search path outside declared source root: {path}") from None

    inputs, stages = {}, []
    for compilation in compilations:
        for path, sha in compilation["inputs"].items():
            key = relative(path)
            require(key not in inputs or inputs[key] == sha, "input changed between stage compilations")
            require(digest(Path(path)) == sha, f"input changed during generation: {path}")
            inputs[key] = sha
        stages.append(dict(source=relative(compilation["source"]), options=compilation["options"],
                           include_dirs=[relative(p) for p in compilation["include_dirs"]],
                           artifact_sha256=compilation["artifact_sha256"]))
    require(output.resolve() != manifest.resolve(), "header and manifest destinations must differ")
    for path in (output, manifest):
        require(str(path.resolve()) not in {p for c in compilations for p in c["inputs"]},
                "generated output would overwrite a source input")
    directory = Path(__file__).resolve().parent
    adapter = {name: digest(directory / name) for name in
               ("generate.py", "layouts.py", "heaps.py", "stages.py", "link_graphics.py", "build_inputs.py")}
    versions = {c["compiler_version"] for c in compilations}
    require(len(versions) == 1, "mixed compiler versions in build receipt")
    record = dict(schema=1, compiler="Slang "+next(iter(versions)), inputs=dict(sorted(inputs.items())),
                  adapter=adapter, compilations=stages,
                  outputs={os.path.relpath(output.resolve(), manifest.resolve().parent):
                           hashlib.sha256(content.encode()).hexdigest()})
    return json.dumps(record, indent=2, sort_keys=True)+"\n"


def publish(output, content, args, compilations, stale_message):
    # Validate all receipts before changing either published file. This is not a
    # multi-file atomic transaction; interrupted writes are detected by --check.
    receipt = manifest_content(args.source_root, args.manifest, output, content, compilations) if args.manifest else None
    if args.check:
        require(output.is_file(), "missing generated interface; regenerate")
        require(output.read_text() == content, stale_message)
        if receipt is not None:
            require(args.manifest.is_file(), "missing build manifest; regenerate")
            require(args.manifest.read_text() == receipt, "stale build inputs/outputs; regenerate")
    else:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(content)
        if receipt is not None:
            args.manifest.parent.mkdir(parents=True, exist_ok=True)
            args.manifest.write_text(receipt)
