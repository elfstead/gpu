"""Independent scalar reference and deterministic scenes; Python stdlib only."""
import math
import struct

CHANNELS = 8
PARAMETERS = 89
TRAIN_SEEDS = tuple(range(41, 73))
QUALITY_CASES = tuple((1001 + i, 33 if i < 4 else 65, 25 if i < 4 else 47)
                      for i in range(8))
EDGE_SHAPES = ((1, 1), (1, 17), (19, 1), (33, 25), (65, 47))


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def packed(values):
    return struct.pack(f"<{len(values)}f", *values)


class Random:
    """Specified xorshift32, not Python's version-dependent sampling helpers."""
    def __init__(self, seed):
        if not 0 < seed <= 0xffffffff:
            raise ValueError("seed must be a nonzero uint32")
        self.state = seed

    def word(self):
        x = self.state
        x ^= (x << 13) & 0xffffffff
        x ^= x >> 17
        x ^= (x << 5) & 0xffffffff
        self.state = x
        return x

    def uniform(self):
        return (self.word() >> 8) / 16777216.0


def clamp(value):
    return min(1.0, max(0.0, value))


def scene(seed, width, height):
    """Sloped background plus five rectangles; independent, unclipped input noise.

    Clean pixels are in [0,1]. Noisy input may be outside that range. Rounding
    inputs to FP32 here ensures training/reference/GPU see identical values.
    """
    if width <= 0 or height <= 0:
        raise ValueError("positive dimensions required")
    rng = Random(seed)
    base = 0.15 + 0.7 * rng.uniform()
    dx, dy = ((rng.uniform() - 0.5) * 0.5 for _ in range(2))
    clean = [clamp(base + dx * (x / max(width - 1, 1) - 0.5)
                   + dy * (y / max(height - 1, 1) - 0.5))
             for y in range(height) for x in range(width)]
    for _ in range(5):
        x0, x1 = sorted((rng.word() % width, rng.word() % width))
        y0, y1 = sorted((rng.word() % height, rng.word() % height))
        level = 0.05 + 0.9 * rng.uniform()
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                clean[y * width + x] = level
    clean = [f32(v) for v in clean]
    noisy = [f32(v + 0.1 * sum(rng.uniform() - 0.5 for _ in range(12)))
             for v in clean]
    return clean, noisy


def check_image(values, width, height):
    if width <= 0 or height <= 0 or len(values) != width * height:
        raise ValueError("image extent/length mismatch")
    if not all(math.isfinite(v) for v in values):
        raise ValueError("nonfinite image")


def check_weights(weights):
    if len(weights) != PARAMETERS or not all(math.isfinite(v) for v in weights):
        raise ValueError("expected 89 finite weights")
    if any(f32(v) != v for v in weights):
        raise ValueError("weights must be stored FP32 values")


def patch(values, width, height, x, y):
    return [values[min(height - 1, max(0, y + ky)) * width
                   + min(width - 1, max(0, x + kx))]
            for ky in (-1, 0, 1) for kx in (-1, 0, 1)]


def infer(weights, noisy, width, height):
    """FP64 scalar oracle over stored FP32 data; returns hidden and denoised.

    Deliberately not the trainer's forward routine or a GPU-kernel translation.
    Hidden is pixel-major with eight contiguous channels. No FP32 intermediate
    rounding: the declared GPU tolerance accounts for FP32 execution error.
    """
    check_weights(weights)
    check_image(noisy, width, height)
    hidden, denoised = [], []
    for y in range(height):
        for x in range(width):
            neighborhood = patch(noisy, width, height, x, y)
            features = []
            for channel in range(CHANNELS):
                value = weights[72 + channel]
                for index, sample in enumerate(neighborhood):
                    value += (sample - 0.5) * weights[channel * 9 + index]
                features.append(max(0.0, value))
            hidden.extend(features)
            correction = weights[88]
            for channel, value in enumerate(features):
                correction += value * weights[80 + channel]
            denoised.append(clamp(noisy[y * width + x] + correction))
    return hidden, denoised


def box_filter(noisy, width, height):
    check_image(noisy, width, height)
    return [clamp(sum(patch(noisy, width, height, x, y)) / 9.0)
            for y in range(height) for x in range(width)]


def render_reference(gray, width, height, out_width, out_height):
    """Half-pixel bilinear resize, affine palette, round-to-nearest RGBA8."""
    check_image(gray, width, height)
    if out_width <= 0 or out_height <= 0:
        raise ValueError("positive output dimensions required")
    rgba, pixels = [], bytearray()
    for y in range(out_height):
        sy = (y + 0.5) * height / out_height - 0.5
        iy = math.floor(sy)
        fy = sy - iy
        y0, y1 = max(0, min(height - 1, iy)), max(0, min(height - 1, iy + 1))
        for x in range(out_width):
            sx = (x + 0.5) * width / out_width - 0.5
            ix = math.floor(sx)
            fx = sx - ix
            x0, x1 = max(0, min(width - 1, ix)), max(0, min(width - 1, ix + 1))
            top = gray[y0 * width + x0] * (1 - fx) + gray[y0 * width + x1] * fx
            bottom = gray[y1 * width + x0] * (1 - fx) + gray[y1 * width + x1] * fx
            value = top * (1 - fy) + bottom * fy
            color = (0.95 * value, 0.8 * value + 0.05, 0.6 * value + 0.15, 1.0)
            rgba.extend(color)
            pixels.extend(math.floor(clamp(c) * 255 + 0.5) for c in color)
    return rgba, bytes(pixels)


def mse(actual, expected):
    if not actual or len(actual) != len(expected):
        raise ValueError("nonempty equal-length arrays required")
    if not all(math.isfinite(v) for v in (*actual, *expected)):
        raise ValueError("nonfinite comparison")
    return sum((a - b) ** 2 for a, b in zip(actual, expected)) / len(actual)


def quality(weights):
    rows = []
    totals = [0.0, 0.0, 0.0]
    pixels = 0
    for seed, width, height in QUALITY_CASES:
        clean, noisy = scene(seed, width, height)
        _, output = infer(weights, noisy, width, height)
        errors = [mse(v, clean) for v in (noisy, box_filter(noisy, width, height), output)]
        rows.append(dict(seed=seed, width=width, height=height,
                         noisy_mse=errors[0], box_mse=errors[1], cnn_mse=errors[2]))
        for i, error in enumerate(errors):
            totals[i] += error * len(clean)
        pixels += len(clean)
    gain = 10 * math.log10(totals[0] / totals[2])
    versus_box = 10 * math.log10(totals[1] / totals[2])
    passed = (gain >= 3.0 and versus_box >= -0.25
              and all(row["cnn_mse"] < row["noisy_mse"] for row in rows))
    return dict(cases=rows, pixels=pixels, gain_db=gain, versus_box_db=versus_box,
                passed=passed)
