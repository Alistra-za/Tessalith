# BC1 on packed `_rmaos` maps: does it flatten channels? (2026-11-22)

For anyone deciding whether Tessalith should get a "packed" BC1 mode for channel-packed maps such as CS PBR `_rmaos` (R roughness, G metallic, B AO, A specular). Measured on `main` at 3212aefd: `bcenc` BC1 (rgbcx level 18), a port of dxt_cpt's per-channel refit, and bc7e `veryslow` as the default's reference. The BC1 encoder itself is covered in [BC1 and BC3](encoder-benchmark.md#bc1-and-bc3).

Short answer:
- **Yes, rgbcx-style BC1 flattens channels, a little.** Of the 4x4 blocks where a channel varies by 8 or more, `bcenc` turns 3.8 % (roughness), 1.3 % (metallic) and 5.5 % (AO) into a single flat value. At a range of 32 or more it is 0.75 %, 0.06 % and 0.5 %.
- **dxt_cpt's refit stops the flattening and makes the map worse.** It flattens 0.4 %, 0.05 % and 0.8 % of those blocks, but its error is higher on 146 of 149 roughness files, 76 of 84 metallic files and every AO file, by SSIM and by RMSE. It is worse even on the blocks `bcenc` flattens (RMSE 6.53 against 5.87 on roughness).
- **BC7, the `_rmaos` default, has about 2.7x less RMSE than `bcenc`** on every channel, and flattens 0.02 to 0.03 % of blocks.
- **Verdict: not worth a plan.** A packed BC1 mode would trade a few flat blocks for more error everywhere, and would cost its own encode version. Leave `_rmaos` on BC7.

## What you'd use this for

If you pick BC1 for an `_rmaos` to save space, this page is the cost. BC1 is half the size of BC7 and has about 2.7x the error per channel on these maps; the flat blocks are a small part of that error. Tessalith allows BC1 on an `_rmaos` only when the alpha it writes is 255 everywhere (the default specular), because BC1 has no data alpha (DECISIONS FM39). The default stays BC7, linear, with the alpha kept as data (`presets/sdc-base.toml`).

## How BC1 can flatten one channel

A BC1 block stores two RGB565 endpoints and a 2-bit index per texel, so every texel lands on one of 4 points along a single line through RGB space. The encoder points that line along the block's main colour variation. In a packed map the channels are unrelated data. When roughness swings hard and AO only a little, the line follows roughness, and AO's small variation can sit across the line instead of along it. Every texel then gets the same AO value. That is the "flattened block" this page counts: the channel's range in the source block is at least 8 (or 16, or 32) and its range after decode is 0.

RGB565 adds a floor: a range of 8 is one 5-bit step in R or B, so part of the range-8 tier is quantisation, not the line.

## What dxt_cpt does differently

dxt_cpt (richgel999/dxt_cpt, `dxtencode.c`, MIT) is stb_dxt-style: a principal axis, then up to 20 rounds of a least-squares endpoint fit with the indices fixed. Its one change is in that fit. Each channel's 16 values are sorted on their own, the index weights are sorted too, and the k-th weight is paired with the k-th value of every channel. So each channel is fitted as if it rose monotonically along the line. That keeps each channel's full range in the endpoints. The final indices are still a plain nearest-colour pick in RGB, so the range is kept but values can land on the wrong texels. The table below shows the second effect wins.

It has no 3-colour mode, no channel weights and no error check: the last fit is used even when it is worse.

## Method

### Data and encoders

- **Sample.** 150 `_rmaos` PNGs drawn from the 2,525 in `B:\testdata\textures-10k`. These are 1024 x 1024 copies made from installed mods, not the authors' native-size sources, so the absolute rates below hold for this corpus. Seed 20261122: the file list sorted case-insensitively, then Python's `random.Random(20261122).sample(list, 150)`. There were enough `_rmaos` files, so no `_m` or `_p` maps were added.
- **Encoders.** `bcenc::bc1_encode_block(px, allow_3color = true, perceptual = false)`, the engine's call for an `_rmaos` (level 18). A literal Rust port of dxt_cpt's `encodeDXT1`, made from a fetched transcript of `dxtencode.c` (f32 maths and operation order kept, its OMatch tables, 4-colour blocks only). bc7e `veryslow`, not perceptual, no `exact_opaque`, with the file's real alpha, as the engine encodes an `_rmaos`. Both BC1 encoders get RGB with alpha set to 255.
- **Decoder.** Both BC1 outputs decode with rgbcx's NVIDIA model (`encoders_sys::bc1_decode_block`), because `bcenc` scores against real decoders and emits 3-colour blocks. An ideal round-to-nearest decoder is the cross-check. BC7 decodes with `bcdec_rs`.

### Scores and checks

- **Scores, per channel.** RMSE; SSIM (11-tap Gaussian, sigma 1.5, the usual constants, on the 8-bit channel); flattened blocks at ranges 8, 16 and 32, as a share of blocks that vary that much; "range kept", the mean of decoded range / source range over blocks with range 8 or more; and the RMSE of each encoder on the texels of blocks `bcenc` flattens.
- **Which files count.** A channel is scored only where its range across the whole file is 8 or more. Metallic has a range under 8 in 66 of the 150 files, and those would only add ties.
- **Port check.** No dxt_cpt binary was built, so the port is checked three ways: every OMatch entry reproduces its index within 1 level, solid blocks decode within 2 levels, and 20,000 random blocks are all 4-colour. On RGB the port's RMSE is 3.52 against `bcenc`'s 3.24, a ratio of 1.087. dxt_cpt's published comparison on its 761 Poly Haven ARM maps, as quoted when this study was set up (not re-read here), is 3.67 against rgbcx's 3.38, a ratio of 1.086.

148 of the 150 files decode with alpha 255 everywhere (a missing alpha channel counts as 255), so Tessalith would accept them as BC1 today. On that subset the means move by 0.06 RMSE and 0.0004 SSIM at most, and the flat-block rates by 0.01 points, so the tables below show all 150.

## Results

Per channel, files where the channel varies (range 8 or more across the file):

| Channel | Files | Encoder | RMSE | SSIM | Flat, range >= 8 | >= 16 | >= 32 | Range kept | RMSE on `bcenc`-flat blocks |
|---|---|---|---|---|---|---|---|---|---|
| R roughness | 149 | `bcenc` | 3.722 | 0.9459 | 3.80 % | 2.11 % | 0.75 % | 0.799 | 5.867 |
| | | dxt_cpt port | 4.048 | 0.9411 | 0.38 % | 0.00 % | 0.00 % | 0.889 | 6.534 |
| | | BC7 | 1.381 | 0.9920 | 0.02 % | 0.00 % | 0.00 % | 0.950 | 1.906 |
| G metallic | 84 | `bcenc` | 2.736 | 0.9832 | 1.34 % | 0.32 % | 0.06 % | 0.762 | 4.281 |
| | | dxt_cpt port | 2.900 | 0.9817 | 0.05 % | 0.00 % | 0.00 % | 0.838 | 4.705 |
| | | BC7 | 0.995 | 0.9947 | 0.03 % | 0.00 % | 0.00 % | 0.936 | 1.794 |
| B AO | 148 | `bcenc` | 3.574 | 0.9581 | 5.45 % | 2.08 % | 0.50 % | 0.794 | 4.825 |
| | | dxt_cpt port | 3.915 | 0.9523 | 0.79 % | 0.01 % | 0.00 % | 0.892 | 5.379 |
| | | BC7 | 1.342 | 0.9928 | 0.03 % | 0.00 % | 0.00 % | 0.944 | 1.666 |

Per file, the port against `bcenc` (Δ = port - `bcenc`, 95 % CI):

| Channel | Files | SSIM: port better / tie / worse | Mean SSIM Δ | RMSE: port better / tie / worse | Mean RMSE Δ |
|---|---|---|---|---|---|
| R roughness | 149 | 3 / 0 / 146 | -0.0048 ± 0.0005 | 0 / 0 / 149 | +0.326 ± 0.022 |
| G metallic | 84 | 4 / 4 / 76 | -0.0015 ± 0.0004 | 1 / 0 / 83 | +0.164 ± 0.026 |
| B AO | 148 | 0 / 0 / 148 | -0.0058 ± 0.0006 | 0 / 0 / 148 | +0.341 ± 0.031 |

The ideal decoder does not change the order: it lowers each mean RMSE by 0.034 or less for both encoders.

### Where the flattening is worst

| File (under `textures-10k`) | Channel | Blocks `bcenc` flattens / port / BC7 (of varying) | SSIM `bcenc` / port / BC7 |
|---|---|---|---|
| `armor\forsworn\forswornarmorf_rmaos.png` | R | 13,167 / 1,515 / 542 of 50,228 | 0.902 / 0.887 / 0.972 |
| `dlc02\dungeons\apocrypha\apocryphabanner01_rmaos.png` | R | 12,689 / 1,495 / 10 of 53,554 | 0.900 / 0.891 / 0.984 |
| `dlc01\dungeons\castle\castlewallstone03_rmaos.png` | B | 12,197 / 1,853 / 12 of 26,973 | 0.909 / 0.894 / 0.988 |
| `dlc02\architecture\ravenrock\redoran\redoran01_rmaos.png` | B | 11,845 / 2,285 / 113 of 38,458 | 0.904 / 0.890 / 0.986 |
| `dungeons\dwemerruins\dwestoneborders03_rmaos.png` | B | 5,977 / 1,888 / 11 of 22,348 | 0.949 / 0.939 / 0.991 |

These are the files a packed mode was meant to help, and the port loses on every one of them. A quarter of `forswornarmorf`'s varying roughness blocks come out flat in `bcenc`, and the port still scores lower.

The port's best files are the two `childrenclothes` maps and `dlc01\clothes\vampire\robes_rmaos.png` on roughness, by 0.001 to 0.01 SSIM. Its RMSE is still higher there (8.24 against 7.73 on `childrenclothesdb`). On those three maps BC7 is at 0.975 to 0.981 where both BC1s sit at 0.73 to 0.83.

## Why the refit loses

This is my reading of the numbers above; I did not measure the mechanism block by block. The refit fixes the endpoints, not the indices. Sorting each channel on its own gives each channel the right range, but the indices still come from one nearest-colour pick in RGB. In a block where roughness and AO do not rise together, a texel's index fits its roughness and then carries an AO value from the other end of the range. A flat block puts every texel at the block's mean, which is often closer than a value picked from the wrong side. That is why the port's error is higher even on exactly the blocks `bcenc` flattens.

Also not measured: a packed BC1 mode that really helped would need a search that scores each channel's error against the indices it actually picks, the way rgbcx's search already does in RGB. With one line and 4 points per block, that search runs into the same limit rgbcx does: the packed channels are unrelated, and BC1 can follow only one direction per block. BC7 has partitions, more index bits and modes with a separate scalar channel, which is why it keeps both roughness and AO.

## What a packed BC1 mode would cost

- **It can never be `bcenc`'s default:** `bcenc` must equal rgbcx byte for byte (DECISIONS HY), so a packed mode would be a second encoder path beside it.
- **Its own encode version:** every `_rmaos` written as BC1 with the mode on gets new bytes, so the `Rmaos` and `SkinRfaos` arms of `encode_version` move and those outputs re-encode once.
- **A setting, its schema entry, GUI row and docs,** plus golden and encode-version pins.
- **A worse result on this data:** by these numbers the setting would raise the error in 146 of 149 roughness files, 76 of 84 metallic files and all 148 AO files (SSIM).

I'd spend that effort elsewhere. A user who wants a better `_rmaos` should keep BC7, which already has 36 to 38 % of BC1's RMSE on every channel.

## Reproduce

The harness is a throwaway example, `crates/bcenc/examples/packed_channels.rs`, in the uncommitted worktree `F:\ai\dds\converter\.worktrees\bc1-packed-research` (it adds `bcdec_rs` as a `bcenc` dev-dependency there). The sampler, the analysis script, the file list, the CSV and the summary are beside it in that worktree's `.claude\scratch\bc1-packed-channels\` (gitignored).

1. `py -3.13 -I sample.py` writes `sample.txt` (seed 20261122, 150 files).
2. In the worktree: `cargo build --offline --release -p bcenc --example packed_channels` with a scratch `CARGO_TARGET_DIR`.
3. `packed_channels.exe sample.txt results.csv` runs in about 55 seconds on 6 threads.
4. `py -3.13 -X utf8 -I analyze.py results.csv > summary.md` prints the tables on this page.

Next: nothing to do for `bcenc`. Revisit only if someone wants a smaller `_rmaos` than BC7 and can live with BC1's error, and then measure a per-channel-scored search, not dxt_cpt's refit.
