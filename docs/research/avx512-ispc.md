# The AVX-512 ISPC path for BC7 and BC6H (2026-11-13)

Whether bc7e (BC7) and the ISPC Texture Compressor (BC6H) should run on ISPC's AVX-512 target on CPUs that have it, what that does to encode time and to the bytes, and which ISPC compiler builds it. Measured on one AMD Ryzen 7 9800X3D on 2026-11-13, from the sources of `main` a10cf405. The ruling is DECISIONS [Note HZ](../../.claude/DECISIONS.md).

Short answer:
- **About 1.5 times as fast.** The `avx512skx-x16` path takes 0.613 to 0.809 of the AVX2 time over 24 configurations, geometric mean 0.673, on one thread. With all 16 logical CPUs busy (measured with the ISPC 1.31.0 build of the path) the ratio was 0.595 to 0.719, mean 0.662.
- **Different bytes, equal quality.** 0.3% to 8.7% of the blocks differ, PSNR moves by at most 0.002 dB, and neither path is better.
- **Two compilers.** ISPC 1.32.0 builds the two AVX-512 objects, because it fixes an AVX-512 miscompile in 1.31.0. ISPC 1.31.0 builds every other file, so the SSE2, SSE4, AVX and AVX2 objects stay the files that were vendored before.
- **Not measured:** an Intel CPU with AVX-512. The tables ran under load; a later run with no build or test running gave the same picture ([Re-checked on the branch](#re-checked-on-the-branch)).

## Method

- **Machine.** AMD Ryzen 7 9800X3D (Zen 5, 8 cores, 16 logical CPUs), Windows. It reports `avx512f`, `avx512dq`, `avx512cd`, `avx512bw` and `avx512vl`, the five extensions ISPC's `avx512skx` target needs, and ISPC's dispatcher picked that target on it. The owner's long test run and other builds ran during every round, so the microseconds below are under load. Read the ratios.
- **Input.** 16 PNGs of 1024×1024 (1,048,576 blocks per configuration), textures of installed mods that can't be shared: `actors\dlc01\shadowmere\shadowmere.png`, `architecture\Markarth\mrkrockdesigns01.png`, `architecture\windhelm\whwoodboards02.png`, `armor\iron\m\bootsplate.png`, `clothes\barkeeper\m\barkeepmaleshoes.png`, `clothes\yarl\m\yarlmaleoutfit_var1.png`, `clutter\common\pickaxe01.png`, `clutter\nightingalecircle01.png`, `creationclub\bgssse001\clutter\books\fishmapthereach01.png`, `dlc01\armor\dwarven\dwarvenshieldcrystal.png`, `dlc02\architecture\telvannitower\telinsideblob01.png`, `dlc02\dungeons\apocrypha\darkmatwall02.png`, `dungeons\ridgedstone08.png`, `landscape\roads\road01snow01.png`, `smim\clutter\imperial\smim_imperial_chains_gray.png`, `weapons\glass\glassarrow.png`.
- **Calls.** The engine's calls: bc7e takes 64 blocks per call, one block row at a time; BC6H takes one 4-row strip per call. Only the encode calls are timed.
- **Timing.** One thread at High priority, pinned to one logical CPU. The two builds alternate per round; the result is the median time and the median of the per-round ratios. Single rounds differ by 1% to 5%.
- **Configurations.** 24 BC7 and BC6H levels and settings; the tables show eight of them, and the ranges cover all 24.
- **Hash.** 64-bit FNV-1a (offset `cbf29ce484222325`, prime `100000001b3`) over every file's output bytes, files in the order above.
  - BC7 input: `image::open(p).to_rgba8()` with alpha set to 255, gathered with `encoders_sys::gather_blocks`. After encoding, `force_opaque_rotated` runs on every block whose 16 source alphas are 255.
  - BC6H input: each 8-bit value k becomes `half::f16::from_f32(srgb_to_linear(k / 255))`, with `srgb_to_linear(c) = c / 12.92` for `c <= 0.04045` and `((c + 0.055) / 1.055).powf(2.4)` above; alpha is `f16::ONE`.

## Speed

Three alternating rounds in one run. The AVX2 column is the ISPC 1.31.0 build in the repository; the AVX-512 column is the shipped ISPC 1.32.0 `avx512skx-x16` build. The last column is the first measurement, with ISPC 1.31.0's own AVX-512 objects and five rounds.

| Configuration | AVX2 µs/block | AVX-512 µs/block | AVX-512 / AVX2 | ISPC 1.31.0 AVX-512 / AVX2 |
|---|---:|---:|---:|---:|
| BC7 veryslow, linear | 28.798 | 18.857 | 0.657 | 0.652 |
| BC7 veryslow, linear, exact_opaque | 27.274 | 17.579 | 0.646 | 0.645 |
| BC7 veryslow, perceptual | 21.459 | 13.138 | 0.613 | 0.614 |
| BC7 fast, linear | 2.691 | 1.769 | 0.658 | 0.669 |
| BC7 max, linear | 76.063 | 49.776 | 0.655 | 0.654 |
| BC6H veryslow + peak (built-in default) | 43.464 | 32.206 | 0.745 | 0.742 |
| BC6H veryslow | 37.269 | 26.128 | 0.701 | 0.705 |
| BC6H fast | 1.437 | 0.951 | 0.664 | 0.659 |
| Geometric mean of all 24 | | | 0.673 | 0.673 |

- Over all 24 configurations the shipped objects take 0.613 to 0.809 of the AVX2 time (1.24 to 1.63 times as fast). ISPC 1.31.0's AVX-512 objects took 0.614 to 0.800; the smallest gains were BC6H veryfast (0.800) and BC7 ultrafast (0.761).
- The machine was busier than in the first measurement: this measurement's own hash passes ran on the other logical CPUs during all three rounds.
- All 16 logical CPUs busy (16 workers, 8 files, ISPC 1.31.0 AVX-512 objects): 0.595 to 0.719, geometric mean 0.662 (1.51 times as fast).

## Bytes and quality

The AVX-512 columns come from the first measurement, with ISPC 1.31.0's own `avx512skx-x16` objects. The shipped ISPC 1.32.0 objects write the same bytes in all 24 configurations ([Which compiler builds the AVX-512 objects](#which-compiler-builds-the-avx-512-objects)), so every figure here holds for them too.

| Configuration | Blocks differing | PSNR AVX2 | PSNR AVX-512 | AVX2 hash | AVX-512 hash |
|---|---:|---:|---:|---|---|
| BC7 veryslow, linear | 69,386 (6.6%) | 44.952 | 44.951 | `94dc2ead09e25c3f` | `e8bdf22de32ceb68` |
| BC7 veryslow, linear, exact_opaque | 72,340 (6.9%) | 44.906 | 44.905 | `f0d943b6f376b66d` | `f2a3db21e7389a30` |
| BC7 veryslow, perceptual | 81,131 (7.7%) | 43.436 | 43.434 | `39bab5573a8c1f25` | `375e1001980289ac` |
| BC7 fast, linear | 8,623 (0.8%) | 43.421 | 43.421 | `7f9901ade6e710a4` | `2ba0443f7b57e193` |
| BC7 max, linear | 76,662 (7.3%) | 45.107 | 45.106 | `79cf3ad2e905fdb0` | `9e08f3458e5668a6` |
| BC6H veryslow + peak (built-in default) | 4,467 (0.4%) | 26.628 | 26.628 | `a3b13499088c3753` | `c78b7e2d6efdd56a` |
| BC6H veryslow | 5,051 (0.5%) | 33.944 | 33.943 | `e78f6e59f5da3f3c` | `22cc3bb8ffad708d` |
| BC6H fast | 13,157 (1.3%) | 33.843 | 33.843 | `77207028ec618567` | `6603d2be59466311` |

- BC7 PSNR is RGB on 8-bit values. BC6H PSNR is on the linear values with peak 1.0. The peak mode's 26.6 dB on this 8-bit-derived input is the same on both paths and was not looked into.
- Bytes differ in all 24 configurations, 0.3% to 8.7% of the blocks. Among the differing blocks the lower error splits about evenly: for BC7 veryslow linear, AVX2 is lower in 32,750, AVX-512 in 30,314, and 6,322 are equal.
- The AVX2 path forced inside the AVX-512 build gave the AVX2 hash in 19 of 19 BC7 and 5 of 5 BC6H configurations, so the AVX2 path writes the same bytes as before. The SSE2, SSE4 and AVX objects are the vendored files themselves.
- Why the bytes differ, inferred from the disassembly and not isolated by a test: the AVX2 objects use `vrcpps` (41 sites in `bc7e_avx2.obj`, 95 in `kernel_avx2.obj`) and the AVX-512 objects use `vrcp14ps` (28 and 81). Both are approximate reciprocals of different precision.

## Other AVX-512 targets tried

ISPC 1.31.0, same input and method as the tables above.

| Target | Bytes against `avx512skx-x16` | Speed |
|---|---|---|
| `avx512icl-x16` | the same | the same within 1% |
| `avx512skx-x8` | the same | 1.11 to 1.18 times as fast as AVX2 for BC7, no faster for BC6H |

Neither is built: they write no other bytes, and neither is faster than `avx512skx-x16`.

## Which compiler builds the AVX-512 objects

ISPC 1.32.0's release notes list a fix for an AVX-512 miscompile in 1.31.0 (ISPC issue 3891): a masked load was moved below an aliasing masked store at `-O1` and `-O2`, which breaks element swaps. The objects are built at `-O2`, and both sources swap under a mask. So the measurement was repeated with ISPC 1.32.0 (LLVM 23.1.2) against ISPC 1.31.0 (LLVM 23.0.0), on the same machine, input and hash.

### Object files

ISPC 1.32.0 against the vendored files (ISPC 1.31.0), the four targets that were vendored before.

| File under `vendor/ispc-objs/` | ISPC 1.31.0 bytes | ISPC 1.32.0 bytes | Same file? |
|---|---:|---:|---|
| `bc7e/bc7e.obj` (dispatcher) | 8,772 | 8,788 | no |
| `bc7e/bc7e_sse2.obj` | 392,989 | 392,877 | no |
| `bc7e/bc7e_sse4.obj` | 351,032 | 351,032 | no |
| `bc7e/bc7e_avx.obj` | 401,468 | 401,468 | yes |
| `bc7e/bc7e_avx2.obj` | 323,870 | 323,854 | no |
| `bc7e/bc7e_ispc.h` | 5,490 | 5,490 | yes |
| `ispctc/kernel.obj` (dispatcher) | 5,451 | 5,467 | no |
| `ispctc/kernel_sse2.obj` | 358,215 | 358,215 | no |
| `ispctc/kernel_sse4.obj` | 353,376 | 353,376 | no |
| `ispctc/kernel_avx.obj` | 419,719 | 419,703 | no |
| `ispctc/kernel_avx2.obj` | 334,607 | 334,591 | no |
| `ispctc/kernel_ispc.h` | 4,629 | 4,629 | yes |

- 9 of the 12 files differ, so `tools\regen-ispc.ps1` run with ISPC 1.32.0 alone cannot report `MATCH` for the vendored objects.
- With five targets, the two generated headers are the same file from both compilers. The two AVX-512 objects and the two dispatcher objects differ between them.
- The dispatchers: ISPC 1.31.0's five-target dispatcher has the same CPU-detection function as the vendored four-target one; only the per-function stubs gain a comparison for the new target. ISPC 1.32.0's detection function differs (one more `cpuid` query).

### Output bytes

Hashes that are the same, out of 24 configurations.

| Compared | Compilers | Same hash |
|---|---|---:|
| Forced SSE2, SSE4 and AVX, each against the vendored objects | 1.32.0 against 1.31.0 | 24 of 24 each |
| Forced AVX2 against the vendored objects | 1.32.0 against 1.31.0 | 24 of 24 |
| Dispatched AVX-512 against dispatched AVX-512 | 1.32.0 against 1.31.0 | 24 of 24 |
| Dispatched AVX-512 against the vendored AVX2 path | 1.32.0 against 1.31.0 | 0 of 24 |
| The shipped mix: forced AVX2 against the vendored objects | mixed against 1.31.0 | 24 of 24 |
| The shipped mix: dispatched against all-1.32.0 and all-1.31.0 AVX-512 | mixed against 1.32.0 and 1.31.0 | 24 of 24 |

- ISPC 1.32.0 changed no output byte on any path, although it changed most object files.
- The repository's pin tests (`level_identity`, `bc6h`, `bc7`, `bc7_fixup`) pass on ISPC 1.32.0's four-target objects.
- The mixed build links without warnings. Every per-target object exports its functions with the target's suffix (`bc7e_compress_blocks_avx512skx`), the dispatcher imports exactly those names, and both compilers use the same names. The AVX-512 objects of both compilers import the same outside symbols.

### Did issue 3891 touch the ISPC 1.31.0 figures?

No sign of it. This is an inference, not a proof:
- The fixed compiler's AVX-512 objects write the same bytes in 24 of 24 configurations. A swap that went wrong would change the output wherever it ran.
- Among the instruction lines that differ between the two compilers' AVX-512 objects, no masked load or masked store changes its order. The only masked moves among them are four masked stores in `bc7e_avx512skx.obj`, which keep their order, address and mask.

Limits: 16 textures; one scalar stack load sits at another place in `kernel_avx512skx.obj`, and its order against the stores around it was not analysed; the difference was counted by instruction name, not read hunk by hunk in `bc7e`; ISPC's issue text was not available offline.

### Why two compilers build the objects

Every file of the four old targets and both dispatcher objects come from ISPC 1.31.0; only `bc7e_avx512skx.obj` and `kernel_avx512skx.obj` come from ISPC 1.32.0. The eight SSE2, SSE4, AVX and AVX2 objects then stay the vendored files byte for byte, so "nothing changed below AVX-512" rests on the files being equal, not on 16 textures hashing equal. The dispatchers keep the CPU-detection code shipped before, instruction for instruction, which matters because a CPU without AVX-512 can't be tested here. The only new code, the AVX-512 path, comes from the compiler without the known AVX-512 bug, and it costs nothing: both compilers' AVX-512 objects write the same bytes at the same speed. `encoders_sys::versions().ispc` stays `"1.31.0"`: it names the compiler of the objects behind every output written before, and a changed value would mark every output stale on every machine.

### The vendored files that are new or changed

Headers after line-2 normalisation (the output path becomes the bare file name).

| File under `vendor/ispc-objs/` | Compiler | Bytes | SHA-256 |
|---|---|---:|---|
| `bc7e/bc7e.obj` (dispatcher, was 8,772 B) | ISPC 1.31.0 | 9,784 | `99d463cbd83924fb4e9bab5af55af859bb3cab832c65fb76ffe3296191faae87` |
| `bc7e/bc7e_ispc.h` (was 5,490 B) | ISPC 1.31.0 | 6,568 | `bea21f0d715221e1ef175aec5dda6c582be4bd939d7a7e1cbea81ec1ec655449` |
| `bc7e/bc7e_avx512skx.obj` (added) | ISPC 1.32.0 | 187,958 | `3e2538006f78c22dc3e020a382633bfda2c98b5e5f2789565c44a33617fee212` |
| `ispctc/kernel.obj` (dispatcher, was 5,451 B) | ISPC 1.31.0 | 5,990 | `d81f7e4d248646d9ee723a8c6dcbf3f6a8a21badd60c697361d3f4f490f88fb3` |
| `ispctc/kernel_ispc.h` (was 4,629 B) | ISPC 1.31.0 | 5,707 | `178e0bd498bbb42a2e94dd285eb7a426c9651a277178a39ca5338b474b9ff51f` |
| `ispctc/kernel_avx512skx.obj` (added) | ISPC 1.32.0 | 247,076 | `13734a9bbf893ee48ed9a55c11f6ce1e113416309b3fb8a8688f41059737c844` |

- The two headers gain one more copy of ISPC's alignment-macro block (26 lines) and no declaration.
- Total object size grows from 2,949,499 to 3,386,084 bytes (+436,585).

## Re-checked on the branch

The branch that added the path ran `ispc_speed` (below) on the same 16 PNGs after vendoring the objects (commit `1e5865dd`) and again after the CLI tests (commit `9c0b09f1`). Another session was building and measuring during both runs, so those ratios are rougher than the tables'. The last column is one run with `--all --rounds 3` on the finished branch, started and ended with no build or test running on the machine (CPU load 19% at the start).

| Configuration | `1e5865dd` | `9c0b09f1` | finished branch, no build running |
|---|---:|---:|---:|
| BC7 veryslow, linear | 0.602 | 0.655 | 0.640 |
| BC7 veryslow, linear, exact_opaque | | | 0.629 |
| BC7 veryslow, perceptual | | | 0.596 |
| BC7 fast, linear | 0.669 | | 0.668 |
| BC7 max, linear | | | 0.623 |
| BC6H veryslow + peak | 0.758 | | 0.717 |
| BC6H veryslow | | | 0.725 |
| BC6H fast | 0.717 | | 0.693 |

- In that last run the eight ratios have a geometric mean of 0.660; the same eight rows of the [Speed](#speed) table give 0.666. BC7 takes 0.60 to 0.67 of the AVX2 time (1.5 to 1.7 times as fast), BC6H 0.69 to 0.73 (1.4 times).

- Every printed hash equalled the [Bytes and quality](#bytes-and-quality) table: the AVX2 path gave the AVX2 hash and the dispatched path the AVX-512 hash.
- `level_identity`'s AVX-512 row differs from its AVX2 row in all 32 cells (8 levels × 4 parameter combinations).

## Not measured

- **An Intel CPU with AVX-512.** The AVX-512 pins were recorded on this AMD Zen 5 CPU only. Whether `vrcp14ps` gives the same bits on Intel is not known, so an Intel CPU may write other AVX-512 bytes (Note HW suspects the same of `vrcpps` on the AVX2 path).
- **The pinned method without load.** The tables' rounds (one thread pinned at High priority) all ran under load. The two pinned measurements agree within 0.011 per configuration in the [Speed](#speed) table; the run without a build used the example's unpinned thread instead.
- **The dispatcher's choice on a CPU without AVX-512.** It can't be run here; a CI runner whose CPU lacks AVX-512 shows it, because `level_identity` encodes through the dispatcher there and compares with the AVX2 row.

## How to repeat it

1. Write a list file with one PNG path per line, the 16 files of [Method](#method) in that order.
2. Run, from the repository root:

   ```powershell
   cargo run --offline --release -p encoders-sys --example ispc_speed -- files.txt --all --rounds 3
   ```

3. Each configuration prints one line: `ISPC <name> blocks=<n> avx2=<µs per block> <hash> best=<target> <µs per block> <hash> ratio=<best / avx2>`. Without `--all` it runs four configurations (BC7 veryslow and fast linear, BC6H veryslow + peak and fast); `--all` adds the other four of the tables.

The example runs on one unpinned thread at normal priority and takes the ratio of the median times, so its ratios scatter more than the tables'. The hashes must match exactly. On a CPU without AVX-512, `best` is `avx2` and the ratio is about 1.
