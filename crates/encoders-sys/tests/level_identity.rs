//! The shipped bc7e levels are byte-identical to the vendored objects. A change to the vendored bc7e source or objects
//! (a new patch, a rebuilt object) must keep every level's output for every flag combination unless a DECISIONS note
//! says why.
//!
//! ISPC builds one code path per target (sse2, avx2, avx512skx) and dispatches on the CPU at run time,
//! and the paths do not encode bit-identically (fast-math, FMA). So the hashes are pinned per target: the test drives
//! every target this CPU can run, up to `ispc_target()`, through `bc7e_compress_blocks_on` (the best one through the
//! dispatcher itself). A CPU without some target skips only that target's rows; a test build caps `ispc_target()`
//! at AVX2, so the AVX-512 row is read only with `SDC_TEST_AVX512=1`. The SSE2 row is also what the SSE4 and AVX
//! paths wrote until Note JB dropped them.
//!
//! The hashes are those of the patch 0005 objects (bc7enc_rdo b943862 + patches 0001, 0002 and 0005; ISPC 1.31.0, the
//! AVX-512 row ISPC 1.32.0), recorded on Windows on an AMD Ryzen 7 9800X3D, 2026-11-23 (Note LR), over four
//! deterministic 64x64 images (opaque colour, cutout, smooth alpha, normal-like) at every level, with and without
//! perceptual weighting and the exact-opaque flag. `RECORDED_S1` pins a fifth image built to reach patch 0005's S1
//! shortcut. The first
//! recording (2026-10-18, one hash per level) came from a Linux CPU's dispatch path and never matched a Windows
//! machine. Run with `SDC_RECORD_LEVEL_HASHES=1` to print the current hashes when a recorded change is intended (needs
//! a DECISIONS note).

use encoders_sys::{
    bc7e_compress_blocks, bc7e_compress_blocks_on, bc7e_compress_blocks_tuned,
    bc7e_compress_blocks_tuned_on, gather_blocks, ispc_target, Bc7Level, Bc7Params, Bc7Tune,
    IspcTarget,
};
use sha2::{Digest, Sha256};

const LEVELS: [Bc7Level; 8] = [
    Bc7Level::Ultrafast,
    Bc7Level::Veryfast,
    Bc7Level::Fast,
    Bc7Level::Basic,
    Bc7Level::Slow,
    Bc7Level::Veryslow,
    Bc7Level::Slowest,
    Bc7Level::Max,
];

/// [target][level][perceptual][exact_opaque] over the four images concatenated; the target rows are SSE2, AVX2 and
/// AVX-512 (`row`). Levels 0 to 6 are the shipped levels; level 7 (`Max`, patch 0002) is checked only when the
/// vendored objects carry the patch.
const RECORDED: [[[[&str; 2]; 2]; 8]; 3] = [
    [
        [
            [
                "1183e8c848d42747fb81cb3fe93e9523c0678c6cf2e222be824e4fd4dbd4487e",
                "a3f059d94cce7ade931d14053ec8ca55dc37cdd36d221cd98a9bd7f88aaa62d3",
            ],
            [
                "b6abb34668fe0743f00096b1600e46e4094636aff6d0d08619b23d208915b5db",
                "b05da6a7b0401d639b4523c94c183849c2bb0b1cafc7c6e55c1acd7b9fd88f17",
            ],
        ],
        [
            [
                "b35db938eb4cb72b43f2bfb03641d62a2fd2b2c16219ecf4b154499db7614110",
                "270d3dbaa3e57fc77cece216e902010869251f6cf91676a66aa934bf8b72efc1",
            ],
            [
                "91c20e0d8a0e2d3f5ff6bc4ccb60dfda85833fa7c232ff64aec168c7a8356aa8",
                "f0a23705301824ad291bb70ab029b07b422b338d7065dd23d2a76fc332003be8",
            ],
        ],
        [
            [
                "9ea2fa3eee3effd3dd654271ad7ef8a1b399979fd87c852bddcc8c014a7975b4",
                "079b152aa100d485e18c4a082667f39705cce1c3ecb93ecd7ba18dd2168be1e6",
            ],
            [
                "91c20e0d8a0e2d3f5ff6bc4ccb60dfda85833fa7c232ff64aec168c7a8356aa8",
                "f0a23705301824ad291bb70ab029b07b422b338d7065dd23d2a76fc332003be8",
            ],
        ],
        [
            [
                "02631180c2db858fa6fc7e7555a5989709ce401852354d67d70b37d61e453ed7",
                "d25c1a06e891e96aab58f0e34056d9ad2014b4efaf612a66205b5f2a0d0864bb",
            ],
            [
                "2e9f058f05fe7009644387cef50daed59f0c4fd73a7940f300282c2d51de8dd6",
                "0c43754ca4475c0ce9c00b8dc1695bbeaafbdab82b45a8812f8607bd5273b8f9",
            ],
        ],
        [
            [
                "226a057dacda0f6bc7a4c3a21638b72d166a8fb22774452eb41090b46845d2ea",
                "2128596bfcf506126d3b8b97ac087c16f4eddb04d20041ebf0e48ad787594cdb",
            ],
            [
                "c8859f1885894b95e7f2826efecbe4cc331d17656200e3461bb65ccbcbc16762",
                "4ed5d3350fa876d75ae38b74ddd54077538668274684baf9eb55f91347b24a01",
            ],
        ],
        [
            [
                "d48131fb061676319723da75e574c6e1ed14fbe3d7167f7282a02908b66dffc8",
                "100a5fb95cb6dbb48c328d2dbe07b32b60f97ec7d02276832c8bd5953d281f3c",
            ],
            [
                "1f8449d76271ba37f8659b3b70c5fdc017ea611abd207670364d241dff4282ef",
                "178389ab9c82e33690e0ac51a6d7a8bfdfbcdccce3969fb0d307afde2ab70161",
            ],
        ],
        [
            [
                "fdb2ae8164607237a17f07a9f9cdca4b1476bf62c2966e9a11abf26ab46c82ed",
                "011018b815b84bbb1ff45f31bb1e856580b77919e28bc932def4c90385f160c2",
            ],
            [
                "59b572ea4b04922094d32cc5c6c46d7451cfb75e4bd99fed583a712db3625d95",
                "e37fd880c5931cf0994a4a571fbac48ed66b0dfa154dddbed3ef2987bbb15f15",
            ],
        ],
        [
            [
                "dda08ce1d73771949a68e3e6ec194abeafba768425c8db975f62a5639864bc16",
                "dd3a555ec32ebd246eca88165a737a55dc521c8274f88e24440fbca5fb02bd85",
            ],
            [
                "58bf80047534217745b9e617096c1658fec8d868ac64ecb25b7e3c9050e648cf",
                "49a9d9a9c0dcd377d3d6e09d4ee5b481a390d659e1085dc95e103ced4cb3d0fd",
            ],
        ],
    ],
    [
        [
            [
                "73a28aacc783aa2674a65ec7d50634c38ace57ef7d35fdef5f47fcfc46abe4b8",
                "6f3c14cb537d6116c88df31266a9d50c17a154950522a997f0ee6d414d2e5b19",
            ],
            [
                "577ecd0994dda1d0d68c9809b25deb51530711128b1d058665d78ab347f0552c",
                "2e49d22691020c2bcdbc67db848711424b4f4f0e7425c285a7c978386f05c2ae",
            ],
        ],
        [
            [
                "a9c2b188b7fea45b8bd893afee53a8672f59efef4866f8817a322c95ad006370",
                "a31204b10b018dd923f30aa42dff058e57f6293f6e03488986035324ac66c136",
            ],
            [
                "33e8c2302dc306c11da150284b5615e104cfa65bd97eebdf07d4d1d8b316d2b7",
                "90c41e6465b7a299b7f44ff04c1d54822b5cd4cfaaf1fbaea79d304ef4c7ecf0",
            ],
        ],
        [
            [
                "21a9216566eb9a55ab505f6b6013ce72ebdf56c274c569099f5f7c87439d85a6",
                "3ccdddb0f12a185f61bc72ed6c6f02148ecaa269b5a07d264a3449f3de9e8ba9",
            ],
            [
                "33e8c2302dc306c11da150284b5615e104cfa65bd97eebdf07d4d1d8b316d2b7",
                "90c41e6465b7a299b7f44ff04c1d54822b5cd4cfaaf1fbaea79d304ef4c7ecf0",
            ],
        ],
        [
            [
                "e12621412fab4f8961e7dc5777c5a0c5d91260aef2d13aa72fe79e589feac0ea",
                "9436205832a83e5c440b82ae641f85389c15e5a39e5ba788d98465e5486cd898",
            ],
            [
                "9e94e39df798b2deaa5480b97bcc52360bf9e9f29325e6d3726c142bd4c91838",
                "7c609ab9225ae4014d282bfb016fedeca0cfa0438c4fafdb2be5e2e3fe498e8f",
            ],
        ],
        [
            [
                "b46f7f838a2b1d55d02a93109d83bac99dae42a00c1005e88c14a4be21b287a8",
                "29a40c250b2b66172025666850a46c81d7978f26b86715b1afab8644ff359781",
            ],
            [
                "944229d6e30909273563737c1cc28b057c227b72f8c7d12d395bcf928bf1c9ec",
                "0b76623762e9eabebef6da39d589551d296cebc9fa400462df4253a847727b8e",
            ],
        ],
        [
            [
                "b3ee13299672041cadc78f2935bbc2df2792909234c564edcfa2123bff18b0bd",
                "c1d68b80c4845559b218b3b981d63b3b40cc77cb20e7eef5a282fd738f0859ef",
            ],
            [
                "e9245fded5b6e9172814e8fb12e7c30b0be2fa0da40da97dfd93a78a61cc645f",
                "c54bd5fd0a5233e7713f1587134dba2f0b644b60892949496f5e5a7c7963480c",
            ],
        ],
        [
            [
                "535a78673abce6ca80743dd9c3ce6e87e3d83692af830ce3519947626f70debb",
                "8e82a86d0578352b269f801c592a94d6ae18cb9e7fff049918ba93df75c30aed",
            ],
            [
                "da994098f2d0f08b1b8d28623d4c6167470c1e6e31f139a163f805e904a340be",
                "196dddaf36a39b0fba87504ffeaeb0e8bbfa3b44c0c5062784379f658be9216e",
            ],
        ],
        [
            [
                "244e923214a40939c1f53b132557134c8579394d91afbdd2ce23e215905db039",
                "8a201aa04bd5b914bf58fb17d5dda948cf35830edd1de0b41679f310aa839451",
            ],
            [
                "d3ac60b401495a087eeb1b371e55b80cc4b54b1c92c80bb147292f6502b644cd",
                "e93997a22003930db0ff49d58c78608420d1b673cdc8ac60132f2ad399d6673b",
            ],
        ],
    ],
    [
        [
            [
                "3617d72f4263c2302fcb2a6e234b819f5ed698ca6df6c9c1655a7a439756f067",
                "e0fa558c2938a8f05d23ab575004febb3b450a9f388a9abce1ba536f29c0ea95",
            ],
            [
                "fa5b25bb750beb17477f15b09c6122f780fee4c72ee40651acfe1ed2a5303a39",
                "dfb2a5d484de80ab8b4cc25720e490278d5243d66ca95c7fcac4c50da4cd49c1",
            ],
        ],
        [
            [
                "efa372af885b78766bbfa9d6573ab25d57056185e4693f88bdb9e9d5ab3d48e0",
                "34af5963be2cd4f915f26f822c3bd481cb10552ff72b7cca7c61948d3a99efcb",
            ],
            [
                "0ea36eca1608f209d50800d8dedc74b9fc064154a9c05197b4c5e699595a32d0",
                "aa290f2f9a70c5afb47ecba1664b03c43df3fdd2eff7453c7406e60f24bf1591",
            ],
        ],
        [
            [
                "5a368ebce84ff549660a72ec574749883a91e3a850a16197196df37281fcd22e",
                "e995ffe1a8f680b8d0f75ce104169db200493b516f6f8383c9b797518e7b9df2",
            ],
            [
                "0ea36eca1608f209d50800d8dedc74b9fc064154a9c05197b4c5e699595a32d0",
                "aa290f2f9a70c5afb47ecba1664b03c43df3fdd2eff7453c7406e60f24bf1591",
            ],
        ],
        [
            [
                "1195bf1d09a3b928f882cbb383d9a20efc589b6ff91871a2a49f6262c7c62b46",
                "18a703de08f1d3c8af47e8deb129a8f0aec11f2acd13e2c96afaa0c82efa8aaa",
            ],
            [
                "21e454a46b708f4e4fefb3d73eb9093c835c190793fadc8ef6da97c4736a7921",
                "eedcd31b8b6027cbf076d34584724c5c9119339262888af52a2aaf5b18d1f76b",
            ],
        ],
        [
            [
                "b4d2b1640bf30a43afd15845c945e6ed0ef2a0bc00a26f9335db8e77159156d2",
                "e7d9bb01533a3c9b13d339f75d27776d54535fe5c12ef9ff6ab76b52fd583792",
            ],
            [
                "6b856866cc902627e43a8748a7d82df4d9fef30625ad8be20b79290b2b8ce45a",
                "c3e4240e4b3cc14d2896be895f3ae777a9a55e2339fe09108ff9b2189ede945c",
            ],
        ],
        [
            [
                "5a9617553f5bd428afb75f1f505be18bf679be663f763a3e34423776dd92db1f",
                "b88e3c95ce9bd1328d4dcb03e1c5667f889587426cab17478ed8083d20f68ab0",
            ],
            [
                "679388854c8262953ee931968cf13ad8ee3fd01370a775fe9065bce01b6cf185",
                "f848c6217804b4a6c77cbfd4275c73dbf352d5ed255392c77c4be395c6683ae9",
            ],
        ],
        [
            [
                "08b05dcdc6abe851a782f655e410e08b896643943b2f58dd8f8bfe31580df1b1",
                "0f36fb54c19620f17af871e717fab4bbc5f5ba9cf6feedabc254882d2878adb6",
            ],
            [
                "44f6af0af63d3b2d9e15bfa1eaab670c8c593d533e4a65deb1212c338ddaf1db",
                "13cffd01298af2f1e451798cab684c3cc3789d170813e8bdd0c3c880d7943e08",
            ],
        ],
        [
            [
                "1a3370c0c40d491c7de7ee758685b0deade709a7dbbbdeb55b07348e44dd6f48",
                "49dc0fb90daa67a8d6b98f126dbfe8b56a9a4a8703146cb627ccab9cda3ba293",
            ],
            [
                "feff5c9bd393e913575e5393af4199c0fc10a7f61b9cf35c0dcc032e2a06a75f",
                "968ea6ea9bb6f84af4878e12bbe332285331a700400575b11e259097b22c4c29",
            ],
        ],
    ],
];

/// The `RECORDED` row of a target.
fn row(target: IspcTarget) -> usize {
    match target {
        IspcTarget::Sse2 => 0,
        IspcTarget::Avx2 => 1,
        IspcTarget::Avx512 => 2,
    }
}

/// A small deterministic generator (xorshift) so the images never depend on a crate.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 32) as u32
    }
    fn byte(&mut self) -> u8 {
        (self.next() & 255) as u8
    }
}

const W: u32 = 64;

fn image(f: impl Fn(u32, u32, &mut Rng) -> [u8; 4]) -> Vec<[u8; 4]> {
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    (0..W * W).map(|i| f(i % W, i / W, &mut r)).collect()
}

/// Opaque colour: two gradients, a sharp edge and noise.
fn colour() -> Vec<[u8; 4]> {
    image(|x, y, r| {
        let edge = if (x / 9 + y / 7) % 3 == 0 { 90 } else { 0 };
        [
            (x * 4).min(255) as u8,
            (y * 4).min(255) as u8,
            (128 + edge + (r.byte() % 24) as u32).min(255) as u8,
            255,
        ]
    })
}

/// Cutout: colour with alpha 0 or 255 and an anti-aliased ring.
fn cutout() -> Vec<[u8; 4]> {
    image(|x, y, r| {
        let dx = x as f64 - 31.5;
        let dy = y as f64 - 31.5;
        let d = (dx * dx + dy * dy).sqrt();
        let a = ((24.0 - d) * 64.0).clamp(0.0, 255.0) as u8;
        [200 - (x as u8 / 2), 60 + (y as u8), r.byte(), a]
    })
}

/// Smooth alpha: a soft blob over varying colour.
fn smooth() -> Vec<[u8; 4]> {
    image(|x, y, _| {
        let dx = x as f64 - 20.0;
        let dy = y as f64 - 40.0;
        let a = (255.0 * (-(dx * dx + dy * dy) / 600.0).exp()) as u8;
        [(x * 3) as u8, (255 - y * 2) as u8, 128, a]
    })
}

/// Normal-like: unit vectors around +Z with ripples, alpha 255.
fn normal() -> Vec<[u8; 4]> {
    image(|x, y, _| {
        let nx = (x as f64 * 0.3).sin() * 0.4;
        let ny = (y as f64 * 0.21).cos() * 0.4;
        let nz = (1.0 - nx * nx - ny * ny).max(0.0).sqrt();
        let q = |v: f64| ((v * 0.5 + 0.5) * 255.0).round() as u8;
        [q(nx), q(ny), q(nz), 255]
    })
}

/// The seed of the S1 candidates (`s1_candidate`).
const S1_SEED: u32 = 0x5105_2026;

/// The candidates whose plain encode is mode 4 at rotation 0 on SSE2, AVX2 and AVX-512 in at least one cell (level
/// basic, slow, veryslow, slowest or max, exact off or on, perceptual off), so the pin below reaches bc7e patch 0005's
/// S1 shortcut (an alpha slot that is 255 on every texel). Chosen by a seeded search on the pre-Q2t objects, which
/// kept a candidate while some cell it covers held fewer than 8; each cell holds 8 to 21 (Note LR, LR2).
const S1_CANDIDATES: [u32; 36] = [
    2148,
    18210,
    19519,
    20927,
    28021,
    29066,
    29667,
    33560,
    43811,
    48272,
    56875,
    67302,
    80410,
    90399,
    115343,
    221307,
    257309,
    263949,
    273845,
    287038,
    296708,
    393400,
    406063,
    0x8000_0000,
    0x8000_0002,
    0x8000_0005,
    0x8000_000a,
    0x8000_000f,
    0x8000_0010,
    0x8000_0011,
    0x8000_0012,
    0x8000_0015,
    0x8000_0026,
    0x8000_002f,
    0x8000_0034,
    0x8000_003b,
];

/// Candidate `k`: an opaque 4x4 block from an LCG seeded with `S1_SEED ^ k * 0x9E37_79B9`. Below bit 31, smooth colour:
/// a base colour, one gradient shared by R, G and B (each scaled by 1 to 3) and noise of -1 to 1. With bit 31 set, a
/// block on mode 4's own palette: 5-bit endpoints per channel, a smooth ramp over the 3-bit weights, and on two
/// candidates in three a -1 to 1 nudge on about a quarter of the texel channels.
fn s1_candidate(k: u32) -> [[u8; 4]; 16] {
    let mut s = S1_SEED ^ k.wrapping_mul(0x9E37_79B9);
    let mut next = move || {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (s >> 24) as i32
    };
    if k & 0x8000_0000 == 0 {
        let base = [next(), next(), next()];
        let (gx, gy) = (next() % 13 - 6, next() % 13 - 6);
        let mul = [next() % 3 + 1, next() % 3 + 1, next() % 3 + 1];
        return std::array::from_fn(|i| {
            let t = gx * (i % 4) as i32 + gy * (i / 4) as i32;
            let mut px = [0, 0, 0, 255];
            for c in 0..3 {
                px[c] = (base[c] + t * mul[c] / 2 + next() % 3 - 1).clamp(0, 255) as u8;
            }
            px
        });
    }
    const W3: [i32; 8] = [0, 9, 18, 27, 37, 46, 55, 64];
    let e5 = |x: i32| (x << 3) | (x >> 2);
    let lo = [e5(next() >> 3), e5(next() >> 3), e5(next() >> 3)];
    let hi = [e5(next() >> 3), e5(next() >> 3), e5(next() >> 3)];
    let (a, b, c) = (next() % 5 - 2, next() % 5 - 2, next() % 8);
    let noisy = next() % 3 != 0;
    std::array::from_fn(|i| {
        let w = W3[(a * (i % 4) as i32 + b * (i / 4) as i32 + c).clamp(0, 7) as usize];
        let mut px = [0, 0, 0, 255];
        for ch in 0..3 {
            let v = ((64 - w) * lo[ch] + w * hi[ch] + 32) >> 6;
            let n = if noisy && next() % 4 == 0 {
                next() % 3 - 1
            } else {
                0
            };
            px[ch] = (v + n).clamp(0, 255) as u8;
        }
        px
    })
}

/// The S1 fixture (BC7-T2-1): `S1_CANDIDATES`' blocks in order, repeated to fill a 64x64 image (256 blocks). A pure
/// function of the constant: it never encodes or filters, so it is the same on every target and after every later
/// kernel change.
fn s1_blocks() -> Vec<[u8; 4]> {
    let mut img = vec![[0u8; 4]; (W * W) as usize];
    for (bi, k) in S1_CANDIDATES.iter().cycle().take(256).enumerate() {
        let (bx, by) = (bi % 16 * 4, bi / 16 * 4);
        for (ti, t) in s1_candidate(*k).into_iter().enumerate() {
            img[(by + ti / 4) * W as usize + bx + ti % 4] = t;
        }
    }
    img
}

fn hash_by(encode: impl Fn(&[[u8; 4]], &mut [u8])) -> String {
    let mut h = Sha256::new();
    for img in [colour(), cutout(), smooth(), normal()] {
        let blocks = gather_blocks(&img, W, W);
        let mut out = vec![0u8; blocks.len()];
        encode(&blocks, &mut out);
        h.update(&out);
    }
    hex::encode(h.finalize())
}

fn hash(level: Bc7Level, perceptual: bool, exact_opaque: bool, target: IspcTarget) -> String {
    let p = Bc7Params {
        level,
        perceptual,
        exact_opaque,
    };
    hash_by(|px, out| bc7e_compress_blocks_on(px, out, p, target))
}

#[test]
fn every_shipped_level_is_byte_identical_to_the_recorded_objects() {
    if encoders_sys::skip_byte_pins() {
        return;
    }
    let record = std::env::var_os("SDC_RECORD_LEVEL_HASHES").is_some();
    let mut diffs = vec![];
    for target in IspcTarget::ALL
        .into_iter()
        .filter(|t| t.supported() && *t <= ispc_target())
    {
        for (li, level) in LEVELS.iter().enumerate() {
            for perceptual in [false, true] {
                for exact in [false, true] {
                    let got = hash(*level, perceptual, exact, target);
                    if record {
                        println!("{target:?} {li} {perceptual} {exact} {got}");
                    }
                    let want = RECORDED[row(target)][li][perceptual as usize][exact as usize];
                    if got != want {
                        diffs.push(format!(
                            "{target:?} {level:?} perceptual={perceptual} exact={exact}: {got}"
                        ));
                    }
                }
            }
        }
    }
    if record {
        eprintln!("SDC_RECORD_LEVEL_HASHES is set: the hashes were printed, nothing was compared");
        return;
    }
    assert!(
        diffs.is_empty(),
        "bc7e output changed for shipped levels (goldens would change):\n{}",
        diffs.join("\n")
    );
}

/// [target][level][exact_opaque] of `s1_blocks()` with perceptual off; the target rows as in `RECORDED`.
const RECORDED_S1: [[[&str; 2]; 8]; 3] = [
    [
        [
            "96c474e3179f36f1114a83083f14f203640637c4b745704edb684ed73b6da447",
            "6a3c08ff7813c32812ee2e5f5766f72a747f15c7eb3320002798a851d9e76b09",
        ],
        [
            "cad02fe53cf73343298b05612c7cf69e946bbf5cdc6451ae19649e261552d1f1",
            "742b9cb3f054eecae3ef36874c29ebffcd6f66df5875ec5d88ae727029621980",
        ],
        [
            "eb09faeeea5b12592560dba44b8eec647ca26dff0ddfb2dcc4b6089525565ce0",
            "7d46777ee5e3d8f57650c5550e05d7bdb8edf7499564f2b00ddc0fb5ea307e65",
        ],
        [
            "bcf38d289476842e8dc886844c950ff035b56aefc0c981287a375f05cb5f7c8f",
            "f317ec45b3895aa2a6b9231a32bb49c4746f8752affa4288c4813ffadddcf25f",
        ],
        [
            "d60587d17689d41494b6b5244741cb2c22995b1f086a1b9a1fd3b855772259b7",
            "913c3622822278000c2d7dfe65654fec4223c2e0965c1404adea7fab83e99a69",
        ],
        [
            "95a51d68235d15f9468a9e53d028cb0a2733644d51f1ec2aea54225f954b0476",
            "286dacb4f27ce1577c7aa2430856c5306d1bb7dd7fe9532d263f8b768211f71d",
        ],
        [
            "57a1fb6feadb637d704af3fa8a163d8111a61a25ee739a262ceb8c46ecda0d1c",
            "6c8cf96edc2bf5fd767acb6713f8c239c6a99f64f85d6ffd7fe707425dac0c59",
        ],
        [
            "c9c4c27dfb10476b6374a23c08676089c145f27331e0612526f9b71a2d4b7213",
            "c3510f804b5877278856860cb334840c388a518b4ba25133a5e0fbdbdc2bdab5",
        ],
    ],
    [
        [
            "96c474e3179f36f1114a83083f14f203640637c4b745704edb684ed73b6da447",
            "6a3c08ff7813c32812ee2e5f5766f72a747f15c7eb3320002798a851d9e76b09",
        ],
        [
            "cad02fe53cf73343298b05612c7cf69e946bbf5cdc6451ae19649e261552d1f1",
            "742b9cb3f054eecae3ef36874c29ebffcd6f66df5875ec5d88ae727029621980",
        ],
        [
            "eb09faeeea5b12592560dba44b8eec647ca26dff0ddfb2dcc4b6089525565ce0",
            "7d46777ee5e3d8f57650c5550e05d7bdb8edf7499564f2b00ddc0fb5ea307e65",
        ],
        [
            "bcf38d289476842e8dc886844c950ff035b56aefc0c981287a375f05cb5f7c8f",
            "f317ec45b3895aa2a6b9231a32bb49c4746f8752affa4288c4813ffadddcf25f",
        ],
        [
            "d60587d17689d41494b6b5244741cb2c22995b1f086a1b9a1fd3b855772259b7",
            "913c3622822278000c2d7dfe65654fec4223c2e0965c1404adea7fab83e99a69",
        ],
        [
            "95a51d68235d15f9468a9e53d028cb0a2733644d51f1ec2aea54225f954b0476",
            "a7baf9f733d623421c4a946c2295f41e4180aded7fe778b5d65c5ac8e0b024b8",
        ],
        [
            "57a1fb6feadb637d704af3fa8a163d8111a61a25ee739a262ceb8c46ecda0d1c",
            "6c8cf96edc2bf5fd767acb6713f8c239c6a99f64f85d6ffd7fe707425dac0c59",
        ],
        [
            "c9c4c27dfb10476b6374a23c08676089c145f27331e0612526f9b71a2d4b7213",
            "c3510f804b5877278856860cb334840c388a518b4ba25133a5e0fbdbdc2bdab5",
        ],
    ],
    [
        [
            "96c474e3179f36f1114a83083f14f203640637c4b745704edb684ed73b6da447",
            "966f9f2f69eaa4e68b0be8e1cb7cc4e2f0601ce65799b17fa82b49e5ecf58582",
        ],
        [
            "33fb82229dfae6ff68fc88ba12f5e8bd539286b290830c015b55f538ff67b420",
            "b84a740bb6c619c14c5881d4496cd0dea37d395880163fb5582624696acdbd27",
        ],
        [
            "96e03b1c13a9a8f4821d609f444882973f1b3217123f5268a5305637cbfa29ee",
            "bea892c123b16f39348ce417df7f648cbdb1026a4029584a7fd207dd1b563fc0",
        ],
        [
            "bcf38d289476842e8dc886844c950ff035b56aefc0c981287a375f05cb5f7c8f",
            "f317ec45b3895aa2a6b9231a32bb49c4746f8752affa4288c4813ffadddcf25f",
        ],
        [
            "d60587d17689d41494b6b5244741cb2c22995b1f086a1b9a1fd3b855772259b7",
            "913c3622822278000c2d7dfe65654fec4223c2e0965c1404adea7fab83e99a69",
        ],
        [
            "a6ac7f6484d083be241b04a9e3e25a6f5ba460d02c30fcf5d8a79654f00323f5",
            "ca9d2a7d416cca6c0dcbffc0e8dea326bf2b376b2051e5ab87f5e57767e85083",
        ],
        [
            "57a1fb6feadb637d704af3fa8a163d8111a61a25ee739a262ceb8c46ecda0d1c",
            "c52e646b9519c759cfc4d0d0016bf3fc8c59e177204ce8450da231bd4beade05",
        ],
        [
            "c9c4c27dfb10476b6374a23c08676089c145f27331e0612526f9b71a2d4b7213",
            "c3510f804b5877278856860cb334840c388a518b4ba25133a5e0fbdbdc2bdab5",
        ],
    ],
];

/// S1 (bc7e patch 0005) is pinned by blocks that reach it: `s1_blocks()` per target, level and exact flag.
#[test]
fn s1_fixture_is_pinned() {
    if encoders_sys::skip_byte_pins() {
        return;
    }
    let record = std::env::var_os("SDC_RECORD_LEVEL_HASHES").is_some();
    let blocks = gather_blocks(&s1_blocks(), W, W);
    let mut diffs = vec![];
    for target in IspcTarget::ALL
        .into_iter()
        .filter(|t| t.supported() && *t <= ispc_target())
    {
        for (li, level) in LEVELS.iter().enumerate() {
            for exact in [false, true] {
                let p = Bc7Params {
                    level: *level,
                    perceptual: false,
                    exact_opaque: exact,
                };
                let mut out = vec![0u8; blocks.len()];
                bc7e_compress_blocks_on(&blocks, &mut out, p, target);
                let got = hex::encode(Sha256::digest(&out));
                if record {
                    println!("S1 {target:?} {li} {exact} {got}");
                }
                if got != RECORDED_S1[row(target)][li][exact as usize] {
                    diffs.push(format!("{target:?} {level:?} exact={exact}: {got}"));
                }
            }
        }
    }
    if record {
        eprintln!("SDC_RECORD_LEVEL_HASHES is set: the hashes were printed, nothing was compared");
        return;
    }
    assert!(
        diffs.is_empty(),
        "bc7e output of the S1 fixture changed:\n{}",
        diffs.join("\n")
    );
}

#[test]
fn tuning_with_no_overrides_matches_the_level_on_every_target() {
    let p = Bc7Params {
        level: Bc7Level::Veryslow,
        perceptual: false,
        exact_opaque: false,
    };
    let blocks = gather_blocks(&colour(), W, W);
    for target in IspcTarget::ALL
        .into_iter()
        .filter(|t| t.supported() && *t <= ispc_target())
    {
        let mut a = vec![0u8; blocks.len()];
        let mut b = vec![0u8; blocks.len()];
        bc7e_compress_blocks_on(&blocks, &mut a, p, target);
        bc7e_compress_blocks_tuned_on(&blocks, &mut b, p, Bc7Tune::default(), target);
        assert_eq!(a, b, "{target:?}");
    }
}

#[test]
fn the_avx512_row_is_not_the_avx2_row() {
    assert_ne!(
        RECORDED[row(IspcTarget::Avx512)],
        RECORDED[row(IspcTarget::Avx2)]
    );
}

/// The engine calls the plain functions. They must encode on `ispc_target()`, whatever ISPC would dispatch to.
/// No pin is read here, so the test cannot fail for the reason HW4 describes.
#[test]
fn the_plain_calls_encode_on_the_process_target() {
    let p = Bc7Params {
        level: Bc7Level::Veryslow,
        perceptual: false,
        exact_opaque: false,
    };
    let want = hash_by(|px, out| bc7e_compress_blocks_on(px, out, p, ispc_target()));
    assert_eq!(
        hash_by(|px, out| bc7e_compress_blocks(px, out, p)),
        want,
        "plain"
    );
    assert_eq!(
        hash_by(|px, out| bc7e_compress_blocks_tuned(px, out, p, Bc7Tune::default())),
        want,
        "tuned"
    );
}

#[test]
fn tuning_with_no_overrides_matches_the_level() {
    let p = Bc7Params {
        level: Bc7Level::Veryslow,
        perceptual: false,
        exact_opaque: false,
    };
    let img = colour();
    let blocks = gather_blocks(&img, W, W);
    let mut a = vec![0u8; blocks.len()];
    let mut b = vec![0u8; blocks.len()];
    bc7e_compress_blocks(&blocks, &mut a, p);
    bc7e_compress_blocks_tuned(&blocks, &mut b, p, Bc7Tune::default());
    assert_eq!(a, b);
}
