"""Check the runtime catalog against the pinned native format definitions."""
from pathlib import Path
import re
import json
import hashlib
import unittest
import xml.etree.ElementTree as ET


ROOT = Path(__file__).resolve().parents[1]


class FormatCatalogTests(unittest.TestCase):
    def test_complete_pinned_registry_disposition(self):
        registry = ROOT / "vendor/Vulkan-Headers/registry/vk.xml"
        manifest = json.loads((ROOT / "tests/format-disposition.json").read_text())
        self.assertEqual(hashlib.sha256(registry.read_bytes()).hexdigest(), manifest["registry_sha256"])
        native = {f.attrib["name"] for f in ET.parse(registry).findall("formats/format")}
        catalog = (ROOT / "crates/ogpu/src/foundation/format_catalog.rs").read_text()
        implemented = set(re.findall(r"VkFormat_(VK_FORMAT_\w+)", catalog))
        remaining = [n for group in manifest["remaining"].values() for n in group]
        self.assertEqual(len(remaining), len(set(remaining)))
        self.assertFalse(implemented & set(remaining))
        self.assertEqual(native, implemented | set(remaining))
        self.assertEqual(len(native), manifest["registry_formats"])
        self.assertEqual(len(implemented), manifest["implemented_formats"])

    def test_native_blocks_and_view_classes(self):
        native = {
            f.attrib["name"]: f.attrib
            for f in ET.parse(ROOT / "vendor/Vulkan-Headers/registry/vk.xml").findall("formats/format")
        }
        text = (ROOT / "crates/ogpu/src/foundation/format_catalog.rs").read_text()
        pattern = re.compile(
            r"(\w+) = (\d+) => \(VkFormat_(\w+), (\d+), (\d+), (\w+), (\d+), \((\d+), (\d+), (\d+)\)\),"
        )
        entries = pattern.findall(text)
        self.assertEqual(len(entries), 190)
        native_classes, ogpu_classes = {}, {}
        compressed = 0
        for name, identity, vk_name, aspects, klass, numeric, depth, x, y, size in entries:
            with self.subTest(format=name):
                f = native[vk_name]
                self.assertEqual(f.get("blockExtent", "1,1,1"), f"{x},{y},1")
                self.assertEqual(native_classes.setdefault(f["class"], klass), klass)
                self.assertEqual(ogpu_classes.setdefault(klass, f["class"]), f["class"])
                if int(aspects) == 1:
                    self.assertEqual(int(size), int(f["blockSize"]))
                    self.assertEqual(int(depth), 0)
                else:
                    self.assertEqual(int(size), 0)
                    self.assertEqual(numeric, "DepthStencil")
                if "compressed" in f:
                    compressed += 1
                    self.assertGreaterEqual(int(identity), 119)
                    self.assertEqual(int(aspects), 1)
                    self.assertEqual(numeric, "Float")
        self.assertEqual(compressed, 68)


if __name__ == "__main__":
    unittest.main()
