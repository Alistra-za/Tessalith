# Research

The measurements behind Tessalith's encoder, mip-filter and coverage choices, by format. Each section gives the date and build it measured. The one-page summary is [Research behind the defaults](../wiki/Research.md); rulings are in [DECISIONS.md](../../.claude/DECISIONS.md).

| Format or topic | Where |
|---|---|
| Method, corpora and metric limits | [Benchmark method](encoder-benchmark.md#method), including [why perceptual metrics don't fit linear data](encoder-benchmark.md#perceptual-metrics-on-linear-data) |
| Format against encoder | [The format matters more than the encoder](encoder-benchmark.md#the-format-matters-more-than-the-encoder) |
| BC7 | [Modes, published data, benchmark, `max`, SSIM/VIF](encoder-benchmark.md#bc7); [bc7e tuning and the `max` level](bc7e-tuning.md); [Windows re-check on Skyrim textures](bc7e-tuning.md#windows-re-check-on-skyrim-textures-2026-10-18); [alpha weights against BC3](bc7e-tuning.md#bc7-alpha-weights-against-bc3-2026-10-21) |
| BC1 and BC3 colour | [Published data, benchmark, decoder model, luma weighting, SSIM/VIF](encoder-benchmark.md#bc1-and-bc3) |
| BC4 family (BC4, BC5, BC3 alpha) | [Benchmark, search radius, normals in BC5 against BC7, Tessalith against the others](encoder-benchmark.md#bc4-and-bc5); [BC4-family block search](bc4-family.md) |
| BC6H | [Published data, benchmark, peak-preserving mode](encoder-benchmark.md#bc6h-hdr) |
| Mips and alpha | [Mip filters, normal maps, data maps, alpha coverage, alpha noise](mipmaps-and-alpha.md); [exact opaque alpha in BC7](encoder-benchmark.md#opaque-bc7-is-not-always-alpha-255); [stray alpha](encoder-benchmark.md#stray-alpha-round-1) |
| RDO, DDS headers, encoder upkeep | [Other findings](encoder-benchmark.md#other-findings) |
| Design and Skyrim rules | [How Tessalith came about](design-overview.md) |
| Spikes and phase records, 2026-10-08 to 13 | [Early spikes and phase records](2026-10-early-spikes.md), moved verbatim from DECISIONS.md |
