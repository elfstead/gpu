#!/usr/bin/env python3
"""Analytical checks independent of the learned parameter values."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
sys.dont_write_bytecode = True
import reference as ref
import train


class ReferenceTests(unittest.TestCase):
    def test_prng_vector(self):
        rng = ref.Random(1)
        self.assertEqual([rng.word() for _ in range(3)], [270369, 67634689, 2647435461])
        for seed in (0, -1, 1 << 32):
            with self.assertRaises(ValueError):
                ref.Random(seed)

    def test_seed_split_and_scenes(self):
        held_out = {case[0] for case in ref.QUALITY_CASES} | {2001, 2002}
        self.assertFalse(held_out.intersection(ref.TRAIN_SEEDS))
        self.assertEqual(ref.scene(1001, 33, 25), ref.scene(1001, 33, 25))
        self.assertNotEqual(ref.scene(2001, 19, 1), ref.scene(2002, 19, 1))
        clean, noisy = ref.scene(2001, 19, 1)
        self.assertTrue(all(0 <= v <= 1 and ref.f32(v) == v for v in clean))
        self.assertTrue(all(ref.f32(v) == v for v in noisy))

    def test_replicated_patch_order(self):
        self.assertEqual(ref.patch([0, 0.25, 0.5, 0.75], 2, 2, 0, 0),
                         [0, 0, 0.25, 0, 0, 0.25, 0.5, 0.5, 0.75])
        self.assertEqual(ref.patch([0.25], 1, 1, 0, 0), [0.25] * 9)

    def test_zero_model_is_clipped_identity(self):
        hidden, result = ref.infer([0.0] * 89, [-0.25, 0.5, 1.25], 3, 1)
        self.assertEqual(hidden, [0.0] * 24)
        self.assertEqual(result, [0, 0.5, 1])

    def test_centering_relu_and_pixel_channel_layout(self):
        weights = [0.0] * 89
        weights[4], weights[72], weights[80] = 1.0, 0.5, -0.5
        hidden, result = ref.infer(weights, [0.25, 0.75], 2, 1)
        self.assertEqual(hidden, [0.25] + [0] * 7 + [0.75] + [0] * 7)
        self.assertEqual(result, [0.125, 0.375])
        weights[72] = 0.0
        hidden, result = ref.infer(weights, [0.25, 0.75], 2, 1)
        self.assertEqual(hidden, [0] * 8 + [0.25] + [0] * 7)
        self.assertEqual(result, [0.25, 0.625])

    def test_last_channel_and_corner_weight(self):
        weights = [0.0] * 89
        weights[63], weights[79], weights[87] = 1.0, 0.5, 0.25
        hidden, result = ref.infer(weights, [0.25, 0.5, 0.75, 0.0], 2, 2)
        self.assertEqual(hidden, ([0] * 7 + [0.25]) * 4)
        self.assertEqual(result, [0.3125, 0.5625, 0.8125, 0.0625])

    def test_box_filter_borders(self):
        self.assertEqual(ref.box_filter([0.25], 1, 1), [0.25])
        actual = ref.box_filter([0, 0.75], 2, 1)
        self.assertEqual(actual, [0.25, 0.5])

    def test_resize_palette_and_quantization(self):
        rgba, pixels = ref.render_reference([0, 1], 2, 1, 1, 1)
        for a, b in zip(rgba, (0.475, 0.45, 0.45, 1)):
            self.assertAlmostEqual(a, b)
        self.assertEqual(pixels, bytes([121, 115, 115, 255]))
        rgba, _ = ref.render_reference([0, 1], 2, 1, 4, 1)
        for x, gray in enumerate((0, 0.25, 0.75, 1)):
            self.assertAlmostEqual(rgba[x * 4], gray * 0.95)
        rgba, pixels = ref.render_reference([0.5], 1, 1, 3, 5)
        self.assertEqual(len(rgba), 60)
        self.assertEqual(pixels, bytes([121, 115, 115, 255]) * 15)

    def test_vertical_resize(self):
        rgba, _ = ref.render_reference([0, 1], 1, 2, 1, 4)
        for y, gray in enumerate((0, 0.25, 0.75, 1)):
            self.assertAlmostEqual(rgba[y * 4], gray * 0.95)

    def test_invalid_data(self):
        for values, width, height in (([], 0, 1), ([1], 2, 1), ([float("nan")], 1, 1)):
            with self.assertRaises(ValueError):
                ref.infer([0.0] * 89, values, width, height)
        for weights in ([0.0] * 88, [float("inf")] * 89, [0.1] * 89):
            with self.assertRaises(ValueError):
                ref.check_weights(weights)
        with self.assertRaises(ValueError):
            ref.render_reference([0.5], 1, 1, 0, 1)
        for a, b in (([], []), ([1], []), ([float("nan")], [0])):
            with self.assertRaises(ValueError):
                ref.mse(a, b)

    def test_model_provenance_rejections(self):
        original = json.loads((train.HERE / "model.json").read_text())
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "model.json"
            for field, value in (("schema", 2), ("model", "unknown"),
                                 ("sources", {}), ("weight_sha256", "bad"),
                                 ("weights", [0.0] * 89), ("training", {})):
                altered = copy.deepcopy(original)
                altered[field] = value
                path.write_text(json.dumps(altered))
                with self.assertRaises(ValueError):
                    train.load(path)

    def test_identity_cannot_pass_quality_gate(self):
        self.assertFalse(ref.quality([0.0] * 89)["passed"])

    def test_frozen_model_quality(self):
        self.assertTrue(ref.quality(train.load(train.HERE / "model.json"))["passed"])


if __name__ == "__main__":
    unittest.main()
