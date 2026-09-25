"""Real pinned compiler dependency graphs and relocatable installed-style checks."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
import generate as g
import link_graphics


class BuildInputTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="ogpu-build-input-test-")
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.root = self.base / "source tree"
        shutil.copytree(g.HERE / "dependencies", self.root)
        self.compiler = os.getenv("SLANGC", "slangc")

    def command(self, root=None, extra=()):
        root = root or self.root
        return [sys.executable, "-B", str(g.HERE / "generate.py"), "--source", str(root / "affine.slang"),
                "--build-dir", str(root / "scratch"), "--name", "affine", "--output", str(root / "affine.generated.h"),
                "--source-root", str(root), "--manifest", str(root / "build.json"), *map(str, extra)]

    def run_tool(self, command, error=None):
        result = subprocess.run(command, capture_output=True, text=True)
        if error:
            self.assertNotEqual(result.returncode, 0, result.stdout+result.stderr)
            self.assertIn(error, result.stdout+result.stderr)
        else: self.assertEqual(result.returncode, 0, result.stdout+result.stderr)
        return result

    def test_nested_include_and_import_graph(self):
        self.run_tool(self.command())
        record = json.loads((self.root / "build.json").read_text())
        self.assertEqual(set(record["inputs"]), {"affine.slang", "affine_math.slang", "include/arguments.slangh",
                                               "include/config.slangh", "include/math.slangh"})
        self.assertEqual(set(record["outputs"]), {"affine.generated.h"})
        self.run_tool(self.command(extra=["--check"]))

    def test_dependency_only_change_with_identical_artifact(self):
        self.run_tool(self.command())
        header = (self.root / "affine.generated.h").read_bytes()
        before = json.loads((self.root / "build.json").read_text())
        path = self.root / "include/math.slangh"
        path.write_text(path.read_text()+"\n// Dependency-only edit: device code remains identical.\n")
        self.run_tool(self.command(extra=["--check"]), "stale build inputs/outputs")
        self.assertEqual((self.root / "affine.generated.h").read_bytes(), header)
        self.run_tool(self.command())
        after = json.loads((self.root / "build.json").read_text())
        self.assertNotEqual(before["inputs"], after["inputs"])
        self.assertEqual(before["compilations"], after["compilations"])
        self.assertEqual((self.root / "affine.generated.h").read_bytes(), header)

    def test_missing_nested_input_does_not_publish(self):
        self.run_tool(self.command())
        header = (self.root / "affine.generated.h").read_bytes()
        receipt = (self.root / "build.json").read_bytes()
        (self.root / "include/config.slangh").rename(self.root / "config.saved")
        self.run_tool(self.command(), "dependency/layout scan failed")
        self.assertEqual((self.root / "affine.generated.h").read_bytes(), header)
        self.assertEqual((self.root / "build.json").read_bytes(), receipt)

    def test_missing_and_stale_outputs(self):
        self.run_tool(self.command())
        output = self.root / "affine.generated.h"
        original = output.read_text()
        output.rename(self.root / "header.saved")
        self.run_tool(self.command(extra=["--check"]), "missing generated interface")
        output.write_text(original+"\n/* stale */\n")
        self.run_tool(self.command(extra=["--check"]), "stale generated interface")
        output.write_text(original)
        (self.root / "build.json").rename(self.root / "manifest.saved")
        self.run_tool(self.command(extra=["--check"]), "missing build manifest")

    def test_relocation(self):
        self.run_tool(self.command())
        relocated = self.base / "another source tree"
        shutil.copytree(self.root, relocated)
        self.run_tool(self.command(relocated, ["--check"]))

    def test_import_search_order(self):
        module = (self.root / "affine_math.slang").read_text().replace('"include/math.slangh"', '"../include/math.slangh"')
        (self.root / "affine_math.slang").rename(self.root / "module.saved")
        for name in ("first", "second"):
            directory = self.root / name
            directory.mkdir()
            (directory / "affine_math.slang").write_text(module)
        first = ["--include-dir", self.root / "first", "--include-dir", self.root / "second"]
        second = ["--include-dir", self.root / "second", "--include-dir", self.root / "first"]
        self.run_tool(self.command(extra=first))
        self.run_tool(self.command(extra=[*second, "--check"]), "stale build inputs/outputs")
        self.run_tool(self.command(extra=second))
        inputs = json.loads((self.root / "build.json").read_text())["inputs"]
        self.assertIn("second/affine_math.slang", inputs)
        self.assertNotIn("first/affine_math.slang", inputs)

    def test_outside_root_and_symlinks(self):
        self.run_tool(self.command())
        before = (self.root / "affine.generated.h").read_bytes()
        config = self.root / "include/config.slangh"
        outside = self.base / "outside.slangh"
        config.rename(outside)
        config.symlink_to(outside)
        self.run_tool(self.command(), "outside declared source root")
        self.assertEqual((self.root / "affine.generated.h").read_bytes(), before)

    def test_output_cannot_overwrite_source(self):
        command = self.command()
        command[command.index("--output")+1] = str(self.root / "affine.slang")
        before = (self.root / "affine.slang").read_bytes()
        self.run_tool(command, "overwrite a source input")
        self.assertEqual((self.root / "affine.slang").read_bytes(), before)

    def test_adapter_and_profile_receipt(self):
        self.run_tool(self.command())
        path = self.root / "build.json"
        record = json.loads(path.read_text())
        self.assertIn("generate.py", record["adapter"])
        record["adapter"]["generate.py"] = "0"*64
        path.write_text(json.dumps(record))
        self.run_tool(self.command(extra=["--check"]), "stale build inputs/outputs")
        self.run_tool(self.command())
        self.run_tool(self.command(extra=["--native-heaps", "--check"]), "stale build inputs/outputs")

    def test_pointee_query_inputs(self):
        source = self.root / "structured.slang"
        source.write_bytes((g.HERE / "structured.slang").read_bytes())
        info = {}
        reflection, _, _ = g.compile_source(source, self.root / "query", self.compiler, build_info=info)
        self.assertIn("ogpuPointeeLayouts", reflection)
        self.assertEqual(set(info["inputs"]), {str(source.resolve())})
        self.assertNotIn("layout-query.slang", " ".join(info["inputs"]))

    def test_graphics_shared_input(self):
        sources = {}
        shared = None
        for stage in ("vertex", "fragment"):
            text = (g.HERE / f"stage_{stage}.slang").read_text()
            split = text.index("};")+2
            if shared is None: shared = text[:split]
            else: self.assertEqual(shared, text[:split])
            sources[stage] = self.root / f"{stage}.slang"
            sources[stage].write_text('#include "varyings.slangh"\n'+text[split:])
        (self.root / "varyings.slangh").write_text(shared)
        infos = []
        link_graphics.header(sources["vertex"], sources["fragment"], self.root / "pair", self.compiler, "pair", build_info=infos)
        for info in infos: self.assertIn(str(self.root / "varyings.slangh"), info["inputs"])
        self.assertEqual(len({p for info in infos for p in info["inputs"]}), 3)

    def test_depfile_parser(self):
        path = self.base / "compiler.d"
        path.write_text("out\\ file: /tmp/a\\ b /tmp/c\\#d /tmp/c$$d \\\n /tmp/e\\:f /tmp/a\\\\b\n")
        self.assertEqual(g.build_inputs.depfile(path), {Path(p) for p in ("/tmp/a b", "/tmp/c#d", "/tmp/c$d", "/tmp/e:f", "/tmp/a\\b")})
        for content in ("out: $(VAR)", "out: a\nother: b", "out: a\\q", "out: a # comment", "not-a-rule"):
            path.write_text(content)
            with self.assertRaises(ValueError): g.build_inputs.depfile(path)

    def test_reported_precompiled_module_rejects(self):
        path = self.root / "opaque.slang-module"
        path.write_bytes(b"not a source module")
        with self.assertRaisesRegex(ValueError, "precompiled modules"):
            g.build_inputs.snapshot([path])

    def test_input_change_during_compilation_rejects(self):
        original_run = g.run
        config = self.root / "include/config.slangh"

        def change_after_compile(*args):
            result = original_run(*args)
            if "-depfile" in args:
                config.write_text(config.read_text()+"\n// concurrent edit\n")
            return result

        with mock.patch.object(g, "run", side_effect=change_after_compile):
            with self.assertRaisesRegex(ValueError, "changed between scan and compilation"):
                g.compile_source(self.root / "affine.slang", self.root / "race", self.compiler, build_info={})


if __name__ == "__main__": unittest.main()
