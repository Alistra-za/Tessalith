//! The shipped bc7e levels are byte-identical to the vendored objects. A change to the vendored bc7e source or objects
//! (a new patch, a rebuilt object) must keep every level's output for every flag combination unless a DECISIONS note
//! says why.
//!
//! ISPC builds one code path per target (sse2, avx2, avx512skx) and dispatches on the CPU at run time,
//! and the paths do not encode bit-identically (fast-math, FMA). So the hashes are pinned per target: the test drives
//! every target this CPU can run, up to `ispc_target()`, through `bc7e_compress_blocks_on` (the best one through the
//! dispatcher itself). A CPU without some target skips only that target's rows; a test build caps `ispc_target()`
//! at AVX2, so the AVX-512 row is read only with `SDC_TEST_AVX512=1`. Until Note JB dropped them, the SSE2 row was
//! also what the SSE4 and AVX paths wrote.
//!
//! The hashes are those of the patch 0005 objects (bc7enc_rdo b943862 + patches 0001, 0002 and 0005), recorded on
//! Windows on an AMD Ryzen 7 9800X3D, 2026-11-23, the exact-opaque rows again on
//! 2026-11-25 when exact opaque became the channel guard's A (Note LR, LR17), and the veryfast and fast rows without
//! perceptual weighting again that day when mode 4 gained the constant-scalar code pair (LR22; the smooth image's B is
//! 128 throughout), and every row without perceptual weighting again that day, from veryfast up, with `RECORDED_S1`,
//! when patch 0005's L16 (outcome D, the direct form) made every linear call rank as the same call with every weight
//! x16 would (LR15), over four deterministic 64x64 images (opaque colour, cutout, smooth alpha, normal-like) at every
//! level, with and without perceptual weighting and exact opaque. `RECORDED_S1` pins a fifth image built to reach patch
//! 0005's S1 shortcut. `X16_RECORDED` holds the same four images' hashes of the tuned x16 calls, recorded once before
//! L16 and now pinning the plain and `[1, 1, 1, N]` calls (`linear_calls_write_the_x16_bytes`). The first recording
//! (2026-10-18, one hash per level) came from a Linux CPU's dispatch path and never matched a Windows machine. Run with
//! `SDC_RECORD_LEVEL_HASHES=1` to print the current hashes when a recorded change is intended (needs a DECISIONS note).
//!
//! Every object is an ISPC 1.32.0 build since 2026-11-26 (Note LU3). The rebuild moved no hash here: the 1.32.0
//! objects write every recorded row, `RECORDED_S1` and `X16_RECORDED` included (LU4).

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
                "dd6ea2089b5fdd01b88357a79d28158e3ce005276f6b20b4691a2a2e14c9b0d3",
            ],
            [
                "b6abb34668fe0743f00096b1600e46e4094636aff6d0d08619b23d208915b5db",
                "7744f3ff9aa3773358caf54a572fbb8c535bd29e0acbbffb082c3fa36526cc03",
            ],
        ],
        [
            [
                "f7e44032884d854ac61675d18cb2cfdb414cb3cd20b67c736387d57db0889143",
                "a9d287e5e6eaea06d75844a48892922a2b793254a2a1b33d4773a0cf025421e9",
            ],
            [
                "96eb3ed4ca8e067f09613d5381ed6171ca625c56d32909898227577bbdc3110b",
                "8b96dc6753754cbd06066f684b476de51b41620460de6cccb9fc82343dcb69fd",
            ],
        ],
        [
            [
                "59ef7a61241b6a42aaf870ecea9531bf0182b016451a7da496d7e6feac23fb44",
                "c7a31f8ca3802ac25d1215f9a8c9226de38f7e55e0ba6d413811186789f25e21",
            ],
            [
                "96eb3ed4ca8e067f09613d5381ed6171ca625c56d32909898227577bbdc3110b",
                "8b96dc6753754cbd06066f684b476de51b41620460de6cccb9fc82343dcb69fd",
            ],
        ],
        [
            [
                "d3d0425a3a2d65833541d0a6a6c3fdaa1ff2fe535e679b00cffde3e4ba4559ae",
                "789f6954719c3eb777130b591a7dbee03196043075d61753375bb632319bff1e",
            ],
            [
                "2e9f058f05fe7009644387cef50daed59f0c4fd73a7940f300282c2d51de8dd6",
                "e9ca4c278b283d5caf06d1281f3abd26821533e7d06503c441ea9346a9c2e1ab",
            ],
        ],
        [
            [
                "6298b5c546c0792a71ee76adf6d91a177042274684c8330700574b736f85e4b0",
                "47c80e3c007c631b7defc1390b9533e33f0c1ceca1ce4f25feaca803e7c0ad92",
            ],
            [
                "c8859f1885894b95e7f2826efecbe4cc331d17656200e3461bb65ccbcbc16762",
                "4ed5d3350fa876d75ae38b74ddd54077538668274684baf9eb55f91347b24a01",
            ],
        ],
        [
            [
                "83af76ffc51b1b8d84c82d65643f8f6495522c256335894b69f28305738f0054",
                "a88d2b492c321fb54a7a96e751a5ec153210c4b1871533c4f1ea00f4db76b04c",
            ],
            [
                "1f8449d76271ba37f8659b3b70c5fdc017ea611abd207670364d241dff4282ef",
                "178389ab9c82e33690e0ac51a6d7a8bfdfbcdccce3969fb0d307afde2ab70161",
            ],
        ],
        [
            [
                "be70df301f8ee176328bd7515d91db6a1383775243ca80b9223458a3b3b4b879",
                "9a99e4a519f1bfd76496bcb5d2aadd35066d16d4eb2fd3a13309e3540034da02",
            ],
            [
                "59b572ea4b04922094d32cc5c6c46d7451cfb75e4bd99fed583a712db3625d95",
                "e37fd880c5931cf0994a4a571fbac48ed66b0dfa154dddbed3ef2987bbb15f15",
            ],
        ],
        [
            [
                "4149d6305cf54c493313aad1daf21222154ac200d175eb1c7550d08e3622a83a",
                "1dfd7425a9ce853230aa1e68894fd9da50b6428eecf09e9148eafaf24667e584",
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
                "9764b7a07aabac0cc92631c98b6795be327439c693b2f23637e7b3f95d4a7aa0",
            ],
            [
                "577ecd0994dda1d0d68c9809b25deb51530711128b1d058665d78ab347f0552c",
                "43ab384b548c816bbf15033c3c2870c95227a9aa510547b62191e91de45f58de",
            ],
        ],
        [
            [
                "b0e211bf5e6a0304339576900af24dc97a5f53a517edfa6e91e3a7cea512fd02",
                "c9fcf629896858c23941efe5ccd1819d434251164a6dad18ea04fa27f6609d7f",
            ],
            [
                "b05416828fee9264e0193e3ec7ee728881c45849b75017666697051645093965",
                "ce8f59924214b72aa8b36dff9e94972c6ab7fbedaedf6a5027eb0625bbd5f473",
            ],
        ],
        [
            [
                "31cc1e36404890331e5f220001dc7e9bca7ffb093e3d48edfe171c930f8e74ea",
                "2de8690f7c43ec3b7614fd3382d8d4fa5707ffca2c0eaa08a113fd838e76d604",
            ],
            [
                "b05416828fee9264e0193e3ec7ee728881c45849b75017666697051645093965",
                "ce8f59924214b72aa8b36dff9e94972c6ab7fbedaedf6a5027eb0625bbd5f473",
            ],
        ],
        [
            [
                "8a86d1e754f574c1536b51612d465a176ebfd3a063cad975d5d3b13274177fd6",
                "c4d1cbda1aebc6238b84874d0343fd9bc126275fa24ef01cabbc254de5cdf3b5",
            ],
            [
                "9e94e39df798b2deaa5480b97bcc52360bf9e9f29325e6d3726c142bd4c91838",
                "5a69a4fd66ac6fb0d585d6344ee7b43057ca1acc74d284780c1f02033f2b6ddd",
            ],
        ],
        [
            [
                "458d7a393a4cb4f95f2a4c4ac1b7310d8e8b795bed830ec452d50f7287cd647f",
                "775a63408d841b7995450545dd3761a34593d0e6eabc532bf66f20d1b8082bc3",
            ],
            [
                "944229d6e30909273563737c1cc28b057c227b72f8c7d12d395bcf928bf1c9ec",
                "0b76623762e9eabebef6da39d589551d296cebc9fa400462df4253a847727b8e",
            ],
        ],
        [
            [
                "6b7c5cca1c432a463bfa049e8e64875d3aa5332148a471135769a931b538a2e1",
                "1df22b15532107badac162db147acebca5c13f0e3afeeb2e0d9153faab301036",
            ],
            [
                "e9245fded5b6e9172814e8fb12e7c30b0be2fa0da40da97dfd93a78a61cc645f",
                "c54bd5fd0a5233e7713f1587134dba2f0b644b60892949496f5e5a7c7963480c",
            ],
        ],
        [
            [
                "c7389ea19fdcace322f7feb565beb1e34decc68fccbc44aa0d0beb55926fe9a0",
                "17c3ce43a6b08b8b50e471fef1b48bd5fc7557cafc1e0e105b9c2c5087eb6597",
            ],
            [
                "da994098f2d0f08b1b8d28623d4c6167470c1e6e31f139a163f805e904a340be",
                "196dddaf36a39b0fba87504ffeaeb0e8bbfa3b44c0c5062784379f658be9216e",
            ],
        ],
        [
            [
                "5ce627bbc3d26d46f3b8f10a237653cae909a0ce3e234b09b950b7807e1d3ced",
                "fe806db967a2287cb0d076402e53bc88174fe81e3d16ee7a1eab35d20c441bc0",
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
                "737a3ebd9c69e121cbab1c360e9de102c8b8316184ca4899efb74bf6a9fc1dc8",
            ],
            [
                "fa5b25bb750beb17477f15b09c6122f780fee4c72ee40651acfe1ed2a5303a39",
                "ba3fbc4576bd3c4e5e09252d3c5711f33b9ded71103765ee26534ee25b04a0af",
            ],
        ],
        [
            [
                "c618f160b946755723cfe52e3b9d48299af72bc81a124fac2ea7373494762b0b",
                "232305140bac08a7c80773342ed33eab81ed797ecf94cc258e97b88abedf5031",
            ],
            [
                "a0bff012a9f525ebf59366f1f83447520604c09f7aa72d3a98ab75fc611dd879",
                "57373d7fc1a2f0012c5a36010399aefa728d56371e614a2626ed643626bed5b3",
            ],
        ],
        [
            [
                "5278d9a2048c83cec5a9368af9fa3cf58c42f652edcf5a5ed7ef7407c4db53b8",
                "ecb9e5b8d72877043faea0c462221d0e336ecce0f1171569c4a4cbe6909d8124",
            ],
            [
                "a0bff012a9f525ebf59366f1f83447520604c09f7aa72d3a98ab75fc611dd879",
                "57373d7fc1a2f0012c5a36010399aefa728d56371e614a2626ed643626bed5b3",
            ],
        ],
        [
            [
                "f3fecec802053a95888c0855c2511431c8e4dca1c3ef97802453de490d6be0d7",
                "4e964bffa0f2e6a98e2931a58d934285ce59c01a131892cf37f4c315f8780f7e",
            ],
            [
                "21e454a46b708f4e4fefb3d73eb9093c835c190793fadc8ef6da97c4736a7921",
                "6ec3c39c6fa315d4e1dac0eb4c54eaffd36f44ed18095969fea14c06a7bf34db",
            ],
        ],
        [
            [
                "c470ea6a0e2ec02f76eb20371aa0d81672fb5cd1d8c4c62fde73d7ca1a1b6ed5",
                "6b6c09c237b43a78b8a80c668be45af9051871086d393c39c4866d7dbf8b87c7",
            ],
            [
                "6b856866cc902627e43a8748a7d82df4d9fef30625ad8be20b79290b2b8ce45a",
                "c3e4240e4b3cc14d2896be895f3ae777a9a55e2339fe09108ff9b2189ede945c",
            ],
        ],
        [
            [
                "4e7207681c5aa13b0d5ed0ef2915c928d81d35c8946e653596352d823c8c882d",
                "67843b41ad60517e36cd2fdb4277495307813a32978c9752ce76e3b56bdb7e9c",
            ],
            [
                "679388854c8262953ee931968cf13ad8ee3fd01370a775fe9065bce01b6cf185",
                "f848c6217804b4a6c77cbfd4275c73dbf352d5ed255392c77c4be395c6683ae9",
            ],
        ],
        [
            [
                "f500d70b7d9c7928f8389ca09e0fff8968a96aeff564b982772041552e2809de",
                "c14db783a43cf792f31c2e33aa81b71afc3e1a6ab8ac19a0b3653407de1b78ee",
            ],
            [
                "44f6af0af63d3b2d9e15bfa1eaab670c8c593d533e4a65deb1212c338ddaf1db",
                "13cffd01298af2f1e451798cab684c3cc3789d170813e8bdd0c3c880d7943e08",
            ],
        ],
        [
            [
                "5e338671ec353834594b78d977dabfc7bd5056b3e307a1363ba5c22cc6a5beb2",
                "6845d36c60f684972d05113c124a282a8b206cb012da1576a745993380a23478",
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

/// Every ISPC path this CPU runs up to `ispc_target()` (Note HZ3): SSE2 and AVX2 in a test build, AVX-512 too with
/// `SDC_TEST_AVX512=1`.
fn paths() -> Vec<IspcTarget> {
    IspcTarget::ALL
        .into_iter()
        .filter(|t| t.supported() && *t <= ispc_target())
        .collect()
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
    for target in paths() {
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
            "fcd5bdcf7433ab3090077f5bda24ea37491db04cd74cc43c0c6ef3b3153c2bb0",
        ],
        [
            "9429506f8336686d3186f6d0cf0a30eadf00abb5afa2f5366e81eb58e5162316",
            "43a89527e53bb13253b963521ca3eaf6999a674e056a35af9873ab6186331ab8",
        ],
        [
            "4bc12b6990553a725f64dcecec7e6d016fd13e097d7891c44084c60ac3a013e1",
            "ad6892498910e3fdb05121a0c1512f10f947e1c3ef549b31af3e5e5b50dfe5e2",
        ],
        [
            "d60587d17689d41494b6b5244741cb2c22995b1f086a1b9a1fd3b855772259b7",
            "92109e1a0e8306848fa9e6697fd676541eeaeb73dff98216c01de824e7951866",
        ],
        [
            "fe0c5bad5e5b14b559e9150cc665b0d9acd2d6ecfcfcc048430f612b591c0bfa",
            "2c8644e0ec94841d0ba6e013a3f923d72f56d2dd6a48c3bb32486870fbef016c",
        ],
        [
            "995b4a5cd50f8672196c7ec9603351640eb0a051bf735118fbec50e12e702b29",
            "72ca230561114216a92ce2be57779d9bb74b8edc87ed29d92bd00e5e6d843aa2",
        ],
        [
            "c7eaddc395912eccc0371a494446063d5641f02f61ac53dd145680a4e97cbadf",
            "08b385c50504b323f7682c3189c4a07228d1ba07be89674821f284a522aa9268",
        ],
    ],
    [
        [
            "96c474e3179f36f1114a83083f14f203640637c4b745704edb684ed73b6da447",
            "6a3c08ff7813c32812ee2e5f5766f72a747f15c7eb3320002798a851d9e76b09",
        ],
        [
            "cad02fe53cf73343298b05612c7cf69e946bbf5cdc6451ae19649e261552d1f1",
            "fcd5bdcf7433ab3090077f5bda24ea37491db04cd74cc43c0c6ef3b3153c2bb0",
        ],
        [
            "9429506f8336686d3186f6d0cf0a30eadf00abb5afa2f5366e81eb58e5162316",
            "43a89527e53bb13253b963521ca3eaf6999a674e056a35af9873ab6186331ab8",
        ],
        [
            "4bc12b6990553a725f64dcecec7e6d016fd13e097d7891c44084c60ac3a013e1",
            "ad6892498910e3fdb05121a0c1512f10f947e1c3ef549b31af3e5e5b50dfe5e2",
        ],
        [
            "d60587d17689d41494b6b5244741cb2c22995b1f086a1b9a1fd3b855772259b7",
            "92109e1a0e8306848fa9e6697fd676541eeaeb73dff98216c01de824e7951866",
        ],
        [
            "fe0c5bad5e5b14b559e9150cc665b0d9acd2d6ecfcfcc048430f612b591c0bfa",
            "2c8644e0ec94841d0ba6e013a3f923d72f56d2dd6a48c3bb32486870fbef016c",
        ],
        [
            "995b4a5cd50f8672196c7ec9603351640eb0a051bf735118fbec50e12e702b29",
            "72ca230561114216a92ce2be57779d9bb74b8edc87ed29d92bd00e5e6d843aa2",
        ],
        [
            "c7eaddc395912eccc0371a494446063d5641f02f61ac53dd145680a4e97cbadf",
            "08b385c50504b323f7682c3189c4a07228d1ba07be89674821f284a522aa9268",
        ],
    ],
    [
        [
            "96c474e3179f36f1114a83083f14f203640637c4b745704edb684ed73b6da447",
            "966f9f2f69eaa4e68b0be8e1cb7cc4e2f0601ce65799b17fa82b49e5ecf58582",
        ],
        [
            "33fb82229dfae6ff68fc88ba12f5e8bd539286b290830c015b55f538ff67b420",
            "32c6f8307e91fc32ffb0df81d8839517e577ab1cbc238eab430289285122abc8",
        ],
        [
            "2592bcd7d68b738a3d8ad6222da939dcdbbac1c9729adfbecbf4cadc7d9d8610",
            "fc0562ca7c3b911b8b0884dceb4c64dca7d7c29ba460415af38205ea79d2f949",
        ],
        [
            "4bc12b6990553a725f64dcecec7e6d016fd13e097d7891c44084c60ac3a013e1",
            "ad6892498910e3fdb05121a0c1512f10f947e1c3ef549b31af3e5e5b50dfe5e2",
        ],
        [
            "d60587d17689d41494b6b5244741cb2c22995b1f086a1b9a1fd3b855772259b7",
            "92109e1a0e8306848fa9e6697fd676541eeaeb73dff98216c01de824e7951866",
        ],
        [
            "62241d7c881ed75a41cdecbe606f7e7022f5faf37514fe891c846f277ada6a6d",
            "520ac8d2a1c8f99c7d870a27d07ca4cd6f3ffd7d16292ae0dd1351a7344c2617",
        ],
        [
            "8d5822acfdce09388fdfbd581815c850b97ad7ada801145ffa8bf8b7641d716c",
            "a25502de772ffd2620108108f74d7f4664cb21b3accd94483a58bc8841594476",
        ],
        [
            "c7eaddc395912eccc0371a494446063d5641f02f61ac53dd145680a4e97cbadf",
            "08b385c50504b323f7682c3189c4a07228d1ba07be89674821f284a522aa9268",
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
    for target in paths() {
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

/// [target][level][exact_opaque][N] of the tuned call with weights `[16, 16, 16, 16 * N]` (N = 1, 4, 16 and 64,
/// perceptual off) over the four images of `RECORDED`, hashed as `RECORDED` is; the target rows as in `RECORDED`.
/// Recorded once, from those calls on the objects before patch 0005's L16 (2026-11-25), and never again from them:
/// since L16 a linear call ranks as the same call with every weight x16 would, so on this kernel those calls rank at
/// x256. The table is the recorded evidence that L16 writes x16's bytes; its N = 1 rows equal `RECORDED`'s
/// non-perceptual rows by design. A later kernel change that moves these bytes re-records it from the calls
/// `linear_calls_write_the_x16_bytes` asserts (the plain call and `[1, 1, 1, N]`, in its record mode), and the table is
/// then a plain pin (Note LR, LR5, LR15).
const X16_RECORDED: [[[[&str; 4]; 2]; 8]; 3] = [
    [
        [
            [
                "1183e8c848d42747fb81cb3fe93e9523c0678c6cf2e222be824e4fd4dbd4487e",
                "29061f4de78bf94c28044adff54c12d1fc6f419c2683bd7d933ec247d84a6333",
                "b933c164d7bc0e2197dfe17bc416ff8a31c6cae6dcf96765325fa35365f2a1f2",
                "cd90d8fda5b500362c31e2bfd08c3d98234ba8dd788f99a16c86e0fe068381b1",
            ],
            [
                "dd6ea2089b5fdd01b88357a79d28158e3ce005276f6b20b4691a2a2e14c9b0d3",
                "ca461733f6276649c6d8bcf15e14759025b1ccb2e02559c02a42a04bec4be9b9",
                "ebc91896a5fe4d02a1f85ba19921f90d54a74a0e2d825699f4fdf5e800c654b2",
                "23335d305313e3e388750e27234dd7b55aa14f1e923c3f4e411ceff588af019b",
            ],
        ],
        [
            [
                "f7e44032884d854ac61675d18cb2cfdb414cb3cd20b67c736387d57db0889143",
                "2e894d62294168cf6bd6a295b983cb0cfdf83b3355660fde7d1c7dbae074fca9",
                "03c7865e351c4c103433f5112488dc6de7315a4248282998f57f1b6a3ff0f4fe",
                "d37fa1a0b6ca8af88e0093bf2184d5c023cf794f20b6b4f011c9ad438c3f8091",
            ],
            [
                "a9d287e5e6eaea06d75844a48892922a2b793254a2a1b33d4773a0cf025421e9",
                "bd4ba83a9844b2e84115cf22f70d48803b091553e89e3cc5b2f21512601eb9ff",
                "b692dead38ef8edcad77238a7b2e7ade1fbbbbef02afafd107089ce013a44476",
                "548fe9f8620d6071eba19228370bae6d2d717d3491d059560c6ffadbcd9ca4c7",
            ],
        ],
        [
            [
                "59ef7a61241b6a42aaf870ecea9531bf0182b016451a7da496d7e6feac23fb44",
                "338434ab6801ba06a4a89689621be5923ee23171754ac008644cb7e4e49c6506",
                "fe4dad4e8284e47093d4508c41788760fe104a5b71ae167ce92f73eea1bfd474",
                "7a0dfc78854684048b4acfff86956632cf5998fd2a8a6e926ca31377d7d65e4e",
            ],
            [
                "c7a31f8ca3802ac25d1215f9a8c9226de38f7e55e0ba6d413811186789f25e21",
                "868483b170c56326ce71091013ad36dd51bd499a2b63a9248f4fded2c0ce8e73",
                "6415e8cbf2799aaf4ead3f00fea072e7086e2e849df4109b78b562eebeb10642",
                "b6e6c4e3fdd247cbc00f8c793a496291b6303ee9b82c624432e5806cf1a247fa",
            ],
        ],
        [
            [
                "d3d0425a3a2d65833541d0a6a6c3fdaa1ff2fe535e679b00cffde3e4ba4559ae",
                "a177ad4ad8d51a3d717b4b4e56018d50c6b1295327c2da2c255a4cf98fd3b450",
                "4013ecb3b7e6becc3007dcbe86e818a7a8e4e494d61590e27d190bc050e906d2",
                "abefd95434da7124b329ed5ae5a689811ff47d09bd7900665db18f23d3ccae5c",
            ],
            [
                "789f6954719c3eb777130b591a7dbee03196043075d61753375bb632319bff1e",
                "24cd270734e1a42484384721bbb217a9f5ad1ced12c0ff6266c2707aa96bb249",
                "ef4f62c9bfec99a404c44bed8cd3251098f8e2c71ba126dfe13a595a2102ca5f",
                "d5afc945bce7b2fc87b548a77d569982b96e01004133aa0d9cbfe65f9fa614df",
            ],
        ],
        [
            [
                "6298b5c546c0792a71ee76adf6d91a177042274684c8330700574b736f85e4b0",
                "a51c54f4d4a694e95c7609bd81bd9ec4883550385ee5e954cc337ad0cfc5fadf",
                "c6d2bb771f14aca22f13dcb2c14088593c178252a3f5107615df755698924279",
                "4aca7efb68ddc9d015a318930329bb7eee93fa113dd38c6560ccdc059ad8365b",
            ],
            [
                "47c80e3c007c631b7defc1390b9533e33f0c1ceca1ce4f25feaca803e7c0ad92",
                "0084710bd46e3a6a4efe66c96fbd798eaf413c7a043e6ca370acdb0e8dd708ac",
                "9234ab4a51a8eafb469264c51bc880b704196b70e0a6ca7ca441a96af6b0fc51",
                "7c1fce109d4d2c8ee642e7bd7f84ee721e6b3e0da3e6a8e62d7799b06baa2603",
            ],
        ],
        [
            [
                "83af76ffc51b1b8d84c82d65643f8f6495522c256335894b69f28305738f0054",
                "39255c7312fc00e9af585209d4b1046fbdb960fa5a1c6109e56f1c9471b75212",
                "f37f8afed18c63b99f37a073b075aed263ef25b471a6ce6059504a165c27d8fd",
                "a863744c7ef40b9b9c9faa9db3b591ca649ea15c016b60660f86ee1690bfbbd9",
            ],
            [
                "a88d2b492c321fb54a7a96e751a5ec153210c4b1871533c4f1ea00f4db76b04c",
                "d104969d0d0d5a434b72e015ff7edacd9dcc6e924a14dd50dc227e62bb57ee96",
                "449f1209093fe684874c68aa6c6f7eb5f097545b0a019a0d4e99a47666159db5",
                "a722ef4b9b25219c96ca544ae42d447299db1d4fedad938c1a08363b67e70d89",
            ],
        ],
        [
            [
                "be70df301f8ee176328bd7515d91db6a1383775243ca80b9223458a3b3b4b879",
                "711a3bbc28e7370c9c5374fc8ba62d07a72749c34d6cab57140f8920cbbe18ee",
                "5ca0b773c5e9662efa0908bb9cc7aff8ae38e289e600a8c465d54b5bdcf242a7",
                "9d0e85f9155b4dddac10f126b0a6eda8de9ca6245fe35b8ea3b1042d6a884200",
            ],
            [
                "9a99e4a519f1bfd76496bcb5d2aadd35066d16d4eb2fd3a13309e3540034da02",
                "614e792a0a90d7d35e7d689d752ff057707c76198a3e88a9af6526a44ee76c07",
                "943d30176a0d3d52852ec071e3b3822d16b641af1fd52b7128acce3ac44b0fc9",
                "728515f82c9da7e08372ad264fe5b0a4e2f48096af0447a0c0f590eea5a42b29",
            ],
        ],
        [
            [
                "4149d6305cf54c493313aad1daf21222154ac200d175eb1c7550d08e3622a83a",
                "48503dde428a2f3860808d0f4aa51c166feb07e5a53b07733cfd5cfd76eb02b8",
                "9c604d4d36500f80b72a471c77ce179a22d321c7bae19d5fb3a6240ebc20ed93",
                "75c850427d17b7dfee95174eb94b7bf0131886238a134a059e67bc48b70eca23",
            ],
            [
                "1dfd7425a9ce853230aa1e68894fd9da50b6428eecf09e9148eafaf24667e584",
                "70ae569727fe51a7478f67662b38d81cea632a6cc8c50360574712ffe9af19ca",
                "dae2cb29447abac2b75caf722d5c44bb801c291b2985d5edaa91ec7f99f12827",
                "4541157caf18812943344d80d0a4569d89b3a9f32561af0bcf85d7411550f4c7",
            ],
        ],
    ],
    [
        [
            [
                "73a28aacc783aa2674a65ec7d50634c38ace57ef7d35fdef5f47fcfc46abe4b8",
                "1099e0c8ddadc8d618c712b6bf10e5be37099ae9012b8130867e37067fdf569c",
                "256055435ff1db982341427b64af8f1181195a7f5d31c74bc8cfc0a2b4443d15",
                "ecbdb34151c03f014896ebc2bb8cef95a620e5690c3da76804dad2f0cbda3cae",
            ],
            [
                "9764b7a07aabac0cc92631c98b6795be327439c693b2f23637e7b3f95d4a7aa0",
                "2c9c340ff56e330885a588b04a92992c151e5185207e275ab585bb6b1d8a82b2",
                "b7ac713e7d28a928f24b9f2fb5c10ce4657dff8ec05cb211129453dd1ae4aa14",
                "e49e1a3c4012df5a07a8636adba09bb553c27162e39b315367a360eb344da012",
            ],
        ],
        [
            [
                "b0e211bf5e6a0304339576900af24dc97a5f53a517edfa6e91e3a7cea512fd02",
                "f9f52799af72c626cf7f0443d2532ea3ae27229dadd588206d84510aa47def71",
                "fa9af71912976f656d167704fd8516ff64edfcd99bf3dc0554910fda8d17794a",
                "66868e08dfd9b59615af7a9d3fee2d537197ddbe038234198e023ce4e03b5cba",
            ],
            [
                "c9fcf629896858c23941efe5ccd1819d434251164a6dad18ea04fa27f6609d7f",
                "e4203204ee95cdb4ea694c022a99c286ff996d8adb2c7796f79be336d882d7fc",
                "8bfe9adecd415608e625a5642d902e62fabe9db48ebaa84995a21657f7aaca00",
                "260ab6379043b83ca17d53a04539c5b24bab648e6489a287eeb13374d6506444",
            ],
        ],
        [
            [
                "31cc1e36404890331e5f220001dc7e9bca7ffb093e3d48edfe171c930f8e74ea",
                "2cfa2d8b7a8d76452e648d9a69702354c2aa71af5406222755ff956a3af5925a",
                "ec0a409209064b0a099348b6a6d0f921187c5c3a30e9adac3b7365724e879791",
                "20b2bc434ccb4fb7e558ab4f0dc3ecea7ab3ef9c76e47516676ab63669c5b9ed",
            ],
            [
                "2de8690f7c43ec3b7614fd3382d8d4fa5707ffca2c0eaa08a113fd838e76d604",
                "0c78fb851f6750ba57130a2fb1f43e1d57ca1fd7f36dbcfa9e20bf98dc4aaf09",
                "5af78ca8215535a3c726830032264a1b02f37e75384cd7982b21bfb8bdb949d0",
                "cef26a93c88b55898521a7f5814e204384ee736666c7a7a32b793cb5a6a34f4f",
            ],
        ],
        [
            [
                "8a86d1e754f574c1536b51612d465a176ebfd3a063cad975d5d3b13274177fd6",
                "310e2ff8c315867c5909a7c2da43d45c6b197b5577956a19ea7e1d98c0ae29f2",
                "7e57dbb655d591531dbf0f68b91c67b2d3ce346f5a126257632e6b4b91cec097",
                "029a0f378d917b7a84435b8ad701f64a2e7969dc2b131b52ce1e49a7a0a5d607",
            ],
            [
                "c4d1cbda1aebc6238b84874d0343fd9bc126275fa24ef01cabbc254de5cdf3b5",
                "78a9350dd61d549d512fcd3a5857ff7f53e3518084f1e6d729b088a2855e95e8",
                "6b301e31a68591fa2bd7a5e177627567bab77182a5de9fbfe70f3d30f389f885",
                "06ec8240437424f257192c25b2aae2d705a1dc3c55238ba39369e53f0ec46392",
            ],
        ],
        [
            [
                "458d7a393a4cb4f95f2a4c4ac1b7310d8e8b795bed830ec452d50f7287cd647f",
                "6d56f1284845fef87335d7864771674f0db2e90220012153cf109b2cdd08bf2c",
                "d651cc1ddc74051d67617339f2ede736015189961b5d75737317818d2d245529",
                "35d7fbaab66fa463ee1f9a53919d4c360018b17e2dee85c96e17114fc3cd0dbc",
            ],
            [
                "775a63408d841b7995450545dd3761a34593d0e6eabc532bf66f20d1b8082bc3",
                "a514bfe4015f7a03a62e4063d4c16da3781b5ccc321ff9bf12919b6aa07200aa",
                "cc90b7f1b26251c172876aa16f4de90a8e3283c1246afc695050354a0815370f",
                "c5593853ed49c3a3bd7c11d992de913999aeb1ad707d0c6042f86ff9eb51b02a",
            ],
        ],
        [
            [
                "6b7c5cca1c432a463bfa049e8e64875d3aa5332148a471135769a931b538a2e1",
                "0091f3db928e0519c0f41b0f1624c142e143f2de3e8adb8ea026753c89290666",
                "bc21c5acda0c51e600f7012b60ece747f9e96d9ae6d42df7d7e844c103870413",
                "6085e16a7e89fbc3fc966c1145fe8567a13d4b8160df4e8c6af786f934f1883f",
            ],
            [
                "1df22b15532107badac162db147acebca5c13f0e3afeeb2e0d9153faab301036",
                "883f1541e550430806d37964ca1af320bc0377068f1e9cc019ad916b2bf41ae9",
                "226ea36b5434575a6c2a4a592900fe18eca13c649c19dd1f447a349fe635805c",
                "5ff1657c114b2c301d6f894e894a584e45cedc3914be15bfd68e2ce7e79bef0c",
            ],
        ],
        [
            [
                "c7389ea19fdcace322f7feb565beb1e34decc68fccbc44aa0d0beb55926fe9a0",
                "c7c91aa805c1e0a1e0da43a973fb4555ed7aae3aefd6b7bb26e7e965f93446ca",
                "e891fbee5afb8c3427b995f1e8dd0107c27e06e343998af9d18b562e7644cbb6",
                "e40a74c9d0344b792adacccfee41dbf6877973d9264b458e20de0348f1dc90a4",
            ],
            [
                "17c3ce43a6b08b8b50e471fef1b48bd5fc7557cafc1e0e105b9c2c5087eb6597",
                "3a499550528cf6fbfb0b86a06aa003336fed7dbbb55714394e46083320e67b27",
                "1bb8afc64152be22837ffd2e3c6e5614fbed57eefc81167906649e4901153f41",
                "1fc6e74f75a09fdb26fbd71d432b58167566ead15227af426b019761ec86f5cf",
            ],
        ],
        [
            [
                "5ce627bbc3d26d46f3b8f10a237653cae909a0ce3e234b09b950b7807e1d3ced",
                "0547f8dc74f87d8af74dd7dbb87a5ed4c3c2b59cd4215b2e72e1ec23733d9983",
                "8393c41548aef319d7d6bc45546577d80316cd6cef6b3a663cb253adf8509f56",
                "59f456a777d9f4627c0ae4c9628a8b52dbce8e4b89b026d3c7f443e1b1c54004",
            ],
            [
                "fe806db967a2287cb0d076402e53bc88174fe81e3d16ee7a1eab35d20c441bc0",
                "465916db90a7db36fae9a1a5d8f50f2f187689bd3765a9574c8acb8318e7647d",
                "73ee26de073c4e1b9067481916dac7e13124d5f3e4b2fd59a41e307792cfce0e",
                "607020d8c0db005ec8143cc00aa0cae9f7aa194eaab1e5b1fda620494de393c4",
            ],
        ],
    ],
    [
        [
            [
                "3617d72f4263c2302fcb2a6e234b819f5ed698ca6df6c9c1655a7a439756f067",
                "d6171e41cc13fa042550907bcd849bf0a71ae4eef2058c2c5c80d126f906dfa1",
                "4d8ac1c2daa59a8a5d2554b1258b2dccf0358a9483a5ea1acc9ac92f88b5f23a",
                "4781f9c330f36a85b3f4df0d394b50f3f64765683978e1ecce4b63c0d8c16e3d",
            ],
            [
                "737a3ebd9c69e121cbab1c360e9de102c8b8316184ca4899efb74bf6a9fc1dc8",
                "5185cbb2dcc0972984683b8809503a248d731369374a5947fc41897de8531fd9",
                "5cdcf35b91a105174747d0d95046e08972bd57db2869be0fc1d9ef18f8083809",
                "1645a3cc1a6b5e95982675a6254b4adbf279c58e8785561bcf3035e28b56d9a4",
            ],
        ],
        [
            [
                "c618f160b946755723cfe52e3b9d48299af72bc81a124fac2ea7373494762b0b",
                "9e84c2c00c0854e58d52716203c6f1cd4fbd018df15aedd4b1de4cf9afbed813",
                "d983b70613fe603fb82095f0ae9d727b714ba587a55998820debe727d0a7dea9",
                "e533e21b800131e82c72ca0a0a20b91fa19557de766959a4a0337d7afb7758ca",
            ],
            [
                "232305140bac08a7c80773342ed33eab81ed797ecf94cc258e97b88abedf5031",
                "5442eda3423110ce3e26566950ebd3f233fcc1e40fe7b074cf2e050fdc7f364b",
                "4ad3f6c30d6d71f7aecbdd2859fea5dd55cb51b400886c934cf3f4026ffc9936",
                "b63848ef0a87e5008321669886db9cddac7530622e82375beb3fc65b6af6b2d0",
            ],
        ],
        [
            [
                "5278d9a2048c83cec5a9368af9fa3cf58c42f652edcf5a5ed7ef7407c4db53b8",
                "d69dce81c657df3a543b3744dad487c40db36ed00ac8b70daac226dea6f17d46",
                "66420bd425555903f179b1c4a6fced2db715e73e5e989a2fe8c0b07590ac63df",
                "304b7058a0c7ffa9482ccb855d4a374f4983e27419a3986baf7ab4a6766a5ce8",
            ],
            [
                "ecb9e5b8d72877043faea0c462221d0e336ecce0f1171569c4a4cbe6909d8124",
                "279178f7490e04674206ea91f2e25a19124aa4e3029abc5fbc1c68ad3b89f05c",
                "e5019b21f6c4fb01c17a42c68168c6743fa5e77cfa4e3e8a1183639afc3fb502",
                "ce57b05b59a3953cef794bf19d6860ad56bb4abf43e7fc579a96ab02aa43d955",
            ],
        ],
        [
            [
                "f3fecec802053a95888c0855c2511431c8e4dca1c3ef97802453de490d6be0d7",
                "535158971ac31837432c38c22b86c14dd1949c0786319438ab8c2f7dee91642a",
                "01c0265d7e5f05f004e8c5a1c3ecffa93fbcbbf79483755480e784810ab078ae",
                "1b43d7d81b4b31ad1e9b28868f08cb4f069b36bf28eb1ae89c802e60f5251b47",
            ],
            [
                "4e964bffa0f2e6a98e2931a58d934285ce59c01a131892cf37f4c315f8780f7e",
                "44fc3dcc9580ceb30d4c37003082216773628dfd73fb027b8aefb9063fa0afe0",
                "12df0a7a2f7b3e7f0768bc4a6b4a49cb41df310c5df2a13bfc711f2a3aaf4d1d",
                "7196832b1abff38134c7113310c4c77a85f31f0140426a8c610866e4da98c1a3",
            ],
        ],
        [
            [
                "c470ea6a0e2ec02f76eb20371aa0d81672fb5cd1d8c4c62fde73d7ca1a1b6ed5",
                "674b4954778ec6251c6783fc682bc404f3ae605b4c8c19bb386f2175dd6430e1",
                "d5edebe624bffa66c7f238714bf9872cd750d15da549932cab20a05cb3340430",
                "60773d4b3b7f29fe2e27cf9603c3f8b5fa59fdf96debbedbf296aaba241dc3e5",
            ],
            [
                "6b6c09c237b43a78b8a80c668be45af9051871086d393c39c4866d7dbf8b87c7",
                "9e25f770173073e5afa6e14c6190a6b328f62e94ceb0cd25cdec97d48c5a0c18",
                "a39de50cb8a2b4f2a8fc23fa87edce2004a492d936d2a1ffca2040d1e4df7665",
                "650358b8cc7c26225d011e994bcdb71ab10ccadb212c8c93c02eb2db9c61bb43",
            ],
        ],
        [
            [
                "4e7207681c5aa13b0d5ed0ef2915c928d81d35c8946e653596352d823c8c882d",
                "1de5e2f9eb9a1dda5193908ee2a66ec73b9c345298046d2343984b3cbf953cbd",
                "bb8a5c8014917de2c935e69f316bde22e287f5cc643c8b28720bfa3d18611a4a",
                "16fd39270041d63d2ba7349cf190d0548e2094d5d853a90021a0a4f252c470b3",
            ],
            [
                "67843b41ad60517e36cd2fdb4277495307813a32978c9752ce76e3b56bdb7e9c",
                "7b7fcce6b80c9aa831782d61d921fe228821cb395b2a03c41e7fb78c1ce5664a",
                "547c8029c496084505bf6a3d95603b0edcbb090e9c2f1fa94885cc6a5dc41cee",
                "4535e61b30816a1e589967a84194f31cc78e5cf5edac8478c3ebf82c870ea7ce",
            ],
        ],
        [
            [
                "f500d70b7d9c7928f8389ca09e0fff8968a96aeff564b982772041552e2809de",
                "22ee408b2fb03493e43ab68c84ff91596933c938426f4803d8bbfb85f676c7f1",
                "e6f20f0338b8282968c71d44fa3f1745d60037b56adc4afdec5e2dea02f5fccf",
                "31b1b0078f7c28e261a68f2142c117945cdcc677ebb70073154898e7e29e23fd",
            ],
            [
                "c14db783a43cf792f31c2e33aa81b71afc3e1a6ab8ac19a0b3653407de1b78ee",
                "cb4700d8fa7fd115e5e84b3086696ba665d7084db8c2452a91283ce43c798b1b",
                "ee6f33b412714c84ec5b132f15344a7d487cf930237d53c9c2e0a48feb67bd42",
                "5bf030e1794dcdc9b14f4a67aca9f37c6bb5b4fc27ec2e1e7a1b5530401e215b",
            ],
        ],
        [
            [
                "5e338671ec353834594b78d977dabfc7bd5056b3e307a1363ba5c22cc6a5beb2",
                "91e1b7633388f1001fc77f829706d089e837f6974436e006ff3a91c183d22ae1",
                "e6429a448b47d0980b65f15bfdbff9b7dec97e2cb0d392e6c02e72f26c201883",
                "b1d2cab43efea72aac215bda0cebf402bd7ffe6f483eb032ec354b3ba5203301",
            ],
            [
                "6845d36c60f684972d05113c124a282a8b206cb012da1576a745993380a23478",
                "b4e4f991f67dea926c82d0dd1590724c2569562374d306a85f61c2d49710f5ca",
                "2c7e7a166ebaade9459457395d5d420fbda2a63ab0f925726cce0db82988cfbf",
                "19e3c8c50c8ad1491941e4b0c03fc4409225d82d886dc10d3ee77163a6a36d4f",
            ],
        ],
    ],
];

/// Patch 0005, L16 (outcome D, the direct form): a linear call ranks its errors as the same call with every weight x16
/// would. Scaling every integer weight by 16 commutes with every float step, so x16 changes bytes at two sites only:
/// the uber threshold, which a linear call now compares at 1/16, and the partition estimators' cast, which now takes 16
/// times their float error. On every path, every level and exact opaque off and on, the plain call writes
/// `X16_RECORDED`'s N = 1 row (the old `[16, 16, 16, 16]` call) and the tuned `[1, 1, 1, N]` its row for N = 4, 16 and
/// 64 (the old `[16, 16, 16, 16 * N]`). Before L16 those calls ranked at x1 and wrote other bytes from veryfast up
/// (Note LR, LR15).
#[test]
fn linear_calls_write_the_x16_bytes() {
    if encoders_sys::skip_byte_pins() {
        return;
    }
    let record = std::env::var_os("SDC_RECORD_LEVEL_HASHES").is_some();
    let (mut diffs, mut equal) = (vec![], 0);
    for target in paths() {
        for (li, level) in LEVELS.iter().enumerate() {
            for exact in [false, true] {
                let p = Bc7Params {
                    level: *level,
                    perceptual: false,
                    exact_opaque: exact,
                };
                for (ni, n) in [1u32, 4, 16, 64].into_iter().enumerate() {
                    let got = if n == 1 {
                        hash_by(|px, out| bc7e_compress_blocks_on(px, out, p, target))
                    } else {
                        let tune = Bc7Tune {
                            weights: Some([1, 1, 1, n]),
                            ..Bc7Tune::default()
                        };
                        hash_by(|px, out| bc7e_compress_blocks_tuned_on(px, out, p, tune, target))
                    };
                    if record {
                        println!("X16 {target:?} {li} {level:?} {exact} {n} {got}");
                    }
                    if got == X16_RECORDED[row(target)][li][exact as usize][ni] {
                        equal += 1;
                    } else {
                        diffs.push(format!("{target:?} {level:?} exact={exact} N={n}: {got}"));
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
        "linear calls did not write the x16 bytes ({equal} rows equal, {} differ):\n{}",
        diffs.len(),
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
    for target in paths() {
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
