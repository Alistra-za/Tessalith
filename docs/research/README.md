# Research

The measurements behind Tessalith's encoder, mip-filter and coverage choices, by format. Each section gives the date and build it measured. The one-page summary is [Research behind the defaults](../wiki/Research.md); rulings are in [DECISIONS.md](../../.claude/DECISIONS.md).

| Format or topic | Where |
|---|---|
| Method, corpora and metric limits | [Benchmark method](encoder-benchmark.md#method), including [why perceptual metrics don't fit linear data](encoder-benchmark.md#perceptual-metrics-on-linear-data) and the [SSIM and VIF rescoring](encoder-benchmark.md#ssim-and-vif-rescoring-2026-10-21) |
| Format against encoder | [The format matters more than the encoder](encoder-benchmark.md#the-format-matters-more-than-the-encoder) |
| BC7 | [Modes, published data, benchmark, normals, `max`](encoder-benchmark.md#bc7); [bc7e tuning and the `max` level](bc7e-tuning.md); [Windows re-check on Skyrim textures](bc7e-tuning.md#windows-re-check-on-skyrim-textures-2026-10-18); [alpha weights against BC3](bc7e-tuning.md#bc7-alpha-weights-against-bc3-2026-10-21); [a scalar bc7e (spike S4)](design-overview.md#spike-s4-a-scalar-bc7e-2026-10-09) |
| BC1 and BC3 colour | [Published data, benchmark, decoder model, luma weighting](encoder-benchmark.md#bc1-and-bc3) |
| BC4 family (BC4, BC5, BC3 alpha) | [Benchmark, search radius, normals in BC5 against BC7, Tessalith against the others](encoder-benchmark.md#bc4-and-bc5); [BC4-family block search](bc4-family.md) |
| BC6H | [Published data, benchmark, peak-preserving mode](encoder-benchmark.md#bc6h-hdr) |
| Mips and alpha | [Mip filters, normal maps, data maps, alpha coverage, alpha noise](mipmaps-and-alpha.md); [exact opaque alpha in BC7 (spikes S5, S5b, S6)](encoder-benchmark.md#opaque-bc7-is-not-always-alpha-255); [stray alpha](encoder-benchmark.md#stray-alpha-round-1) |
| Other findings | [RDO](encoder-benchmark.md#rate-distortion-optimisation), [DDS container rules](encoder-benchmark.md#dds-container-rules), [settings that change nothing](encoder-benchmark.md#settings-that-change-nothing), [encode time of the Rust port of rgbcx](encoder-benchmark.md#encode-time-of-the-rust-port-of-rgbcx-2026-11-13-note-hy), [NVTT per-level feeding (spikes S7, S7b)](encoder-benchmark.md#nvtt-per-level-feeding-spikes-s7-and-s7b-2026-10-09-and-13), [encoder maintenance](encoder-benchmark.md#encoder-maintenance-october-2026) |
| Design and Skyrim rules | [How Tessalith came about](design-overview.md), including [the CS complex-material gate (spike S8)](design-overview.md#spike-s8-the-cs-complex-material-gate-2026-10-09) and the [phase records](design-overview.md#phase-records-2026-10-09) |
