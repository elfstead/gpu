# Advanced raster composition consumer

Copy this directory outside the checkout. Set `PKG_CONFIG_PATH` to this SDK's
`lib/pkgconfig`, then run `python3 build.py` and `python3 run.py`. Only C11, Python,
pkg-config and the installed runtime are needed; shader artifacts are included.
The run requires native raster, tessellation and geometry support. Missing
capabilities fail explicitly; no emulation or automatic profile downgrade occurs.

Five profiles use the same GPU-generated indexed scene: vertex/fragment control,
geometry, tessellation, both stages, and both stages with suspend/resume between
overlapping depth-tested draws. Base patches use factor two; the blended overlay
uses factor four. Six texture/mip/filter/viewport cases each replay A/B/A at
257×193 and 640×360 (180 frames). `--scale` adds 1280×720.

The independent CPU oracle checks texture, color, depth, generated geometry,
indirect records and guards. Mip-view equivalence and A/B/A repeat are exact.
Geometry matches the control with the existing all-pixel tolerance; tessellation
uses the existing oracle boundary mask for nearest-filter ties, while every
linear-filter color and every depth value remains compared. Reports count large
boundary differences explicitly. Adding geometry after tessellation and adding
continuation must preserve every image byte exactly.

`run-*/report.json` records SDK/source/binary identity, device, all frame receipts,
output hashes and comparisons. Failures leave `complete: false`. Enable native
validation through your Vulkan layer environment. This is correctness evidence,
not performance parity or a pure Vulkan comparison.

Optional artifact regeneration: `SLANGC=/path/to/slangc python3 advanced_shaders.py`
requires Slang 2026.14.1 and `spirv-val`. This fixed consumer recipe validates
SPIR-V and reflected root offsets; it is not a general stage-interface linker.
