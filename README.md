# SPIR-V Image & Sampler Combiner

This is a hacky solution to allow `WGSL` programs compiled by [naga](https://github.com/gfx-rs/wgpu/tree/trunk/naga) to be used for `SDL3` vulkan targets (or other targets which require combined `ImageSamplers` instead of a separate `Image` & `Sampler`).


## Usage

```shell
$ spirv-image-sampler-combiner <inputfile.spv> [-o <outputfile.spv>]
```

If `outputfile` is not specified, a new file will be created with the extension `.modified.spv` next to the input file.


## How it Works

This tool simply discards the `Sampler` binding, and upgrades the existing `Image` binding to a `SampledImage` binding.

This is done by:

1. Replacing any usage of `OpTypeImage` with the associated `OpTypeSampledImage`
2. Replacing usages of `OpSampledImage` (which combines samplers and images) with a direct pass-through of the image (which due to the previous step is already a complete `SampledImage`).
3. Using simple heuristics, replacing newly unused instructions with no-ops.