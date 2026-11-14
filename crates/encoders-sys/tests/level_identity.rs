//! The shipped bc7e levels are byte-identical to the objects that produced the goldens. Any change to the
//! vendored bc7e source or objects (a new patch, a rebuilt object) must keep every existing level's output
//! for every flag combination; only a level added behind a new flag may encode differently.
//!
//! ISPC builds one code path per target (sse2, sse4, avx, avx2, avx512skx) and dispatches on the CPU at run time,
//! and the paths do not encode bit-identically (fast-math, FMA). So the hashes are pinned per target: the test drives
//! every target this CPU can run, up to `ispc_target()`, through `bc7e_compress_blocks_on` (the best one through the
//! dispatcher itself). A CPU without some target skips only that target's rows; a test build caps `ispc_target()`
//! at AVX2, so the AVX-512 row is read only with `SDC_TEST_AVX512=1`.
//!
//! The hashes were recorded on Windows with the vendored objects (ISPC 1.31.0, bc7enc_rdo b943862 + patch
//! 0001; level 7 from the patch-0002 objects; the AVX-512 row with the ISPC 1.32.0 objects, on an AMD Zen 5 CPU,
//! 2026-11-13), over four deterministic 64x64 images (opaque colour, cutout,
//! smooth alpha, normal-like) at every level, with and without perceptual weighting and the exact-opaque flag.
//! The first recording (2026-10-18, one hash per level) came from a Linux CPU's dispatch path and never
//! matched a Windows machine. Run with `SDC_RECORD_LEVEL_HASHES=1` to print the current hashes when a
//! recorded change is intended (needs a DECISIONS note).

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

/// [target][level][perceptual][exact_opaque] over the four images concatenated. Levels 0 to 6 are the shipped
/// levels; level 7 (`Max`, patch 0002) is checked only when the vendored objects carry the patch.
const RECORDED: [[[[&str; 2]; 2]; 8]; 5] = [
    [
        [
            [
                "c81a6efea962a3ed81f33e175dfe1e30c537a1ea241700790c6a0d7a16156f0c",
                "73dc2ae4f9c1000094368774c321069e818d9dc2dfd484223796d375ae1119c5",
            ],
            [
                "96fccf91f97ce76df22774dafc3f5a22989eff99386f62b8664f0c85ed67d67b",
                "06361eb2d5b4a5c46d1507049fac1bd958d6cb3883a7aadbe3c537e2b9d9c4af",
            ],
        ],
        [
            [
                "8a6a6e1016b1e1ad872e99012f42694323ba9843a6b31dd223fde79c211f328c",
                "2ad1a21913fd29b5346afe76915af946a7a0819621e145c2a853a2087e778c0e",
            ],
            [
                "47a9e160fd20acfd1990dab3adbf40d01b98c8bf0275377372392732e3d87111",
                "719e76239739d592e51ad938a9723b114af1bad7e2c0e8d6138fdf8004720b87",
            ],
        ],
        [
            [
                "428e0e6d64c1852ceff611b1caeba5fdd3dba990ceb812fdeb76190e7f640b54",
                "f981547441d1f95352b841ecb134122b7eab544ab079781c65113898449341a3",
            ],
            [
                "47a9e160fd20acfd1990dab3adbf40d01b98c8bf0275377372392732e3d87111",
                "719e76239739d592e51ad938a9723b114af1bad7e2c0e8d6138fdf8004720b87",
            ],
        ],
        [
            [
                "cdc3c24d41640bd9fb02cc61d6f6f68cc6293030d96b95cc6a98f472453e7d89",
                "19b6af8e7cad3553ec5a3816d09205e9081f887bcd0835cb732a57f3161aeea8",
            ],
            [
                "a0fc5573280b033705fa57f8a0897a00a0ae072b0266c2db43e019fe547222a1",
                "30be587cc0fd4c833bb4b4c0f7bf102125063709057068ee14403801ab4cd6a2",
            ],
        ],
        [
            [
                "21da89672858b7e82085464471fad19370350df9b027bbead0cecf050258e7f6",
                "da5a4bcaf78c32e08ba089e1a564db4120509c7bcfc09a7606d70d9731af33ca",
            ],
            [
                "6e1902cce00d57ace763aaae8393eb6bbb9081a9e1dc45cbe72af2161876d490",
                "d0fbccdb0d71f6a9427a7e605c11d5e618edc01bbae4224568deb63e13092ac6",
            ],
        ],
        [
            [
                "25dc07cabfe032f52d9b6e9378d87764559a75c68f03c8dc5bc34cd534de6a02",
                "7201336cdb08d6d0e9ecade5fe65b9dbf51b74a57041856b0c3b057fe9beec31",
            ],
            [
                "6bfb56a78411377afed2dc6ffa3eac1d378c121795006bfdcfcad359bf9eef45",
                "1767d91481365ae5a95692252d2b6aa0723259e74528485118f4c10b61db5af5",
            ],
        ],
        [
            [
                "a2a99d418a25a14430e61a11ee204c3939ce49a8743ac85cb2f8465aa552388c",
                "350730eafc0804f876940ca95be87031a757ba1d09fabf468b43ca0cc23800f0",
            ],
            [
                "92becf6d248c3838fb068c265819f4c392217d51feadc02c70a01fe33a075f99",
                "578bbf28b4b7dd3b3daf847870a176fb123e0262d9f9cddcf76f34c30907e446",
            ],
        ],
        [
            [
                "5c05f11849786ea5eb2ee6d5ca0703ef5c0faeede8b51ca8ab4ba936dd19e994",
                "9ce71331e668547146fb31cf2d847c0bd797a01fd52d70142ee6a1d533e557fb",
            ],
            [
                "6165095a8fbb9e9bde5758ab5915f15c97ce1f6ec86f020e9c107af9b6ede49b",
                "d86fda4542902c772670656ec580f371108b419c6b4cb7771f3a3673a2d06547",
            ],
        ],
    ],
    [
        [
            [
                "c81a6efea962a3ed81f33e175dfe1e30c537a1ea241700790c6a0d7a16156f0c",
                "73dc2ae4f9c1000094368774c321069e818d9dc2dfd484223796d375ae1119c5",
            ],
            [
                "96fccf91f97ce76df22774dafc3f5a22989eff99386f62b8664f0c85ed67d67b",
                "06361eb2d5b4a5c46d1507049fac1bd958d6cb3883a7aadbe3c537e2b9d9c4af",
            ],
        ],
        [
            [
                "8a6a6e1016b1e1ad872e99012f42694323ba9843a6b31dd223fde79c211f328c",
                "2ad1a21913fd29b5346afe76915af946a7a0819621e145c2a853a2087e778c0e",
            ],
            [
                "47a9e160fd20acfd1990dab3adbf40d01b98c8bf0275377372392732e3d87111",
                "719e76239739d592e51ad938a9723b114af1bad7e2c0e8d6138fdf8004720b87",
            ],
        ],
        [
            [
                "428e0e6d64c1852ceff611b1caeba5fdd3dba990ceb812fdeb76190e7f640b54",
                "f981547441d1f95352b841ecb134122b7eab544ab079781c65113898449341a3",
            ],
            [
                "47a9e160fd20acfd1990dab3adbf40d01b98c8bf0275377372392732e3d87111",
                "719e76239739d592e51ad938a9723b114af1bad7e2c0e8d6138fdf8004720b87",
            ],
        ],
        [
            [
                "cdc3c24d41640bd9fb02cc61d6f6f68cc6293030d96b95cc6a98f472453e7d89",
                "19b6af8e7cad3553ec5a3816d09205e9081f887bcd0835cb732a57f3161aeea8",
            ],
            [
                "a0fc5573280b033705fa57f8a0897a00a0ae072b0266c2db43e019fe547222a1",
                "30be587cc0fd4c833bb4b4c0f7bf102125063709057068ee14403801ab4cd6a2",
            ],
        ],
        [
            [
                "21da89672858b7e82085464471fad19370350df9b027bbead0cecf050258e7f6",
                "da5a4bcaf78c32e08ba089e1a564db4120509c7bcfc09a7606d70d9731af33ca",
            ],
            [
                "6e1902cce00d57ace763aaae8393eb6bbb9081a9e1dc45cbe72af2161876d490",
                "d0fbccdb0d71f6a9427a7e605c11d5e618edc01bbae4224568deb63e13092ac6",
            ],
        ],
        [
            [
                "25dc07cabfe032f52d9b6e9378d87764559a75c68f03c8dc5bc34cd534de6a02",
                "7201336cdb08d6d0e9ecade5fe65b9dbf51b74a57041856b0c3b057fe9beec31",
            ],
            [
                "6bfb56a78411377afed2dc6ffa3eac1d378c121795006bfdcfcad359bf9eef45",
                "1767d91481365ae5a95692252d2b6aa0723259e74528485118f4c10b61db5af5",
            ],
        ],
        [
            [
                "a2a99d418a25a14430e61a11ee204c3939ce49a8743ac85cb2f8465aa552388c",
                "350730eafc0804f876940ca95be87031a757ba1d09fabf468b43ca0cc23800f0",
            ],
            [
                "92becf6d248c3838fb068c265819f4c392217d51feadc02c70a01fe33a075f99",
                "578bbf28b4b7dd3b3daf847870a176fb123e0262d9f9cddcf76f34c30907e446",
            ],
        ],
        [
            [
                "5c05f11849786ea5eb2ee6d5ca0703ef5c0faeede8b51ca8ab4ba936dd19e994",
                "9ce71331e668547146fb31cf2d847c0bd797a01fd52d70142ee6a1d533e557fb",
            ],
            [
                "6165095a8fbb9e9bde5758ab5915f15c97ce1f6ec86f020e9c107af9b6ede49b",
                "d86fda4542902c772670656ec580f371108b419c6b4cb7771f3a3673a2d06547",
            ],
        ],
    ],
    [
        [
            [
                "c81a6efea962a3ed81f33e175dfe1e30c537a1ea241700790c6a0d7a16156f0c",
                "73dc2ae4f9c1000094368774c321069e818d9dc2dfd484223796d375ae1119c5",
            ],
            [
                "96fccf91f97ce76df22774dafc3f5a22989eff99386f62b8664f0c85ed67d67b",
                "06361eb2d5b4a5c46d1507049fac1bd958d6cb3883a7aadbe3c537e2b9d9c4af",
            ],
        ],
        [
            [
                "8a6a6e1016b1e1ad872e99012f42694323ba9843a6b31dd223fde79c211f328c",
                "2ad1a21913fd29b5346afe76915af946a7a0819621e145c2a853a2087e778c0e",
            ],
            [
                "47a9e160fd20acfd1990dab3adbf40d01b98c8bf0275377372392732e3d87111",
                "719e76239739d592e51ad938a9723b114af1bad7e2c0e8d6138fdf8004720b87",
            ],
        ],
        [
            [
                "428e0e6d64c1852ceff611b1caeba5fdd3dba990ceb812fdeb76190e7f640b54",
                "f981547441d1f95352b841ecb134122b7eab544ab079781c65113898449341a3",
            ],
            [
                "47a9e160fd20acfd1990dab3adbf40d01b98c8bf0275377372392732e3d87111",
                "719e76239739d592e51ad938a9723b114af1bad7e2c0e8d6138fdf8004720b87",
            ],
        ],
        [
            [
                "cdc3c24d41640bd9fb02cc61d6f6f68cc6293030d96b95cc6a98f472453e7d89",
                "19b6af8e7cad3553ec5a3816d09205e9081f887bcd0835cb732a57f3161aeea8",
            ],
            [
                "a0fc5573280b033705fa57f8a0897a00a0ae072b0266c2db43e019fe547222a1",
                "30be587cc0fd4c833bb4b4c0f7bf102125063709057068ee14403801ab4cd6a2",
            ],
        ],
        [
            [
                "21da89672858b7e82085464471fad19370350df9b027bbead0cecf050258e7f6",
                "da5a4bcaf78c32e08ba089e1a564db4120509c7bcfc09a7606d70d9731af33ca",
            ],
            [
                "6e1902cce00d57ace763aaae8393eb6bbb9081a9e1dc45cbe72af2161876d490",
                "d0fbccdb0d71f6a9427a7e605c11d5e618edc01bbae4224568deb63e13092ac6",
            ],
        ],
        [
            [
                "25dc07cabfe032f52d9b6e9378d87764559a75c68f03c8dc5bc34cd534de6a02",
                "7201336cdb08d6d0e9ecade5fe65b9dbf51b74a57041856b0c3b057fe9beec31",
            ],
            [
                "6bfb56a78411377afed2dc6ffa3eac1d378c121795006bfdcfcad359bf9eef45",
                "1767d91481365ae5a95692252d2b6aa0723259e74528485118f4c10b61db5af5",
            ],
        ],
        [
            [
                "a2a99d418a25a14430e61a11ee204c3939ce49a8743ac85cb2f8465aa552388c",
                "350730eafc0804f876940ca95be87031a757ba1d09fabf468b43ca0cc23800f0",
            ],
            [
                "92becf6d248c3838fb068c265819f4c392217d51feadc02c70a01fe33a075f99",
                "578bbf28b4b7dd3b3daf847870a176fb123e0262d9f9cddcf76f34c30907e446",
            ],
        ],
        [
            [
                "5c05f11849786ea5eb2ee6d5ca0703ef5c0faeede8b51ca8ab4ba936dd19e994",
                "9ce71331e668547146fb31cf2d847c0bd797a01fd52d70142ee6a1d533e557fb",
            ],
            [
                "6165095a8fbb9e9bde5758ab5915f15c97ce1f6ec86f020e9c107af9b6ede49b",
                "d86fda4542902c772670656ec580f371108b419c6b4cb7771f3a3673a2d06547",
            ],
        ],
    ],
    [
        [
            [
                "f4f881e665d3d0cb6a28be103a4e65d9dfa4ce1a00b9ec96971bbef7c102b521",
                "f773dcf5079e25acf30dc7eeff7a911e993b113439e21150875f01d05469d872",
            ],
            [
                "15418e10638a57727d6e31730addb23d7bf973d81ea8d41d745bf34a9f574b20",
                "d67fd028bf8b91de4214063c7371aa71a2379d8c4b808d9fb8679a44e6fcfb52",
            ],
        ],
        [
            [
                "3ff869de12b06fee3d3ae90fdcdfb3ec1889d7fc1031fe5dc3502de15523cbdf",
                "7af80a08149b79116d0def7ccd8b72adad5a7e2bdcf01f578df8e6517852a582",
            ],
            [
                "7780d167b53895404aa7613a5b8836c1a61be667128ff0a4d2b59117a77a8d9a",
                "40f4f566536dc8b50facad435f19f95b6cac9c28070d9ebad78afd63d5cbbd06",
            ],
        ],
        [
            [
                "cc2254b2373ae003321acd7c41c85991e80e41173711b957cc303ae02c2814cf",
                "a5e9fd09896007b56e574926b0cf1a7be3a0a0785c1f20aacc8fa076d0a7e0b8",
            ],
            [
                "7780d167b53895404aa7613a5b8836c1a61be667128ff0a4d2b59117a77a8d9a",
                "40f4f566536dc8b50facad435f19f95b6cac9c28070d9ebad78afd63d5cbbd06",
            ],
        ],
        [
            [
                "bb2149c8ce1a4e0ccf1bdd36ec39133128c10734fe91155183564c7c70f6f879",
                "4a4a47c893f0a17f8260c0dfb873625a02e26ff59b46809f765feacd93d1607a",
            ],
            [
                "5b7c106a2e6ca600d50e6d858697f1cb760503f92b4f894bd1cfa20c754bf60a",
                "fb856616184236945cebe26e76ebb42a2adb3399f62dfd56b1d5ee0ee3ce712d",
            ],
        ],
        [
            [
                "9d16b273c5cf8d588da5fb4ba2c0f39aff1f3c73f21c0310dbb74217173a46b8",
                "5febe256a2850591c647fc3d01f7efcd6a3443c6087fe8db9153a00fdfaaf256",
            ],
            [
                "ec9628a4d53d61ae265c06baa12398f30ae7aa35aa7b8023c3ab4a50f366a95f",
                "724c0e894395e9489ba6370bd11b149767301d3afd29249788c6def9db3bd98a",
            ],
        ],
        [
            [
                "b068539b97d4bf132561824c97b6bd0d9517e4a8f949ad143b57c9ae47dfc080",
                "ecfebbf1a859f2cf96a5222c33cadf330d11de74e6002df000a5fb6bd38f92ff",
            ],
            [
                "ee28db2921dea588bb1fd61ff6c7d0c2d9d64f5dea05ee4623cb5ccdaf7981ad",
                "5fa4bc74f31e80a65ca7e08c3213fcc6ddd4e87747f46b4e3de86a951aa67849",
            ],
        ],
        [
            [
                "7c90b6c626c19155b4af29f8d3caee578d80afcd3876419e9fd7f24023e864a5",
                "0fea99f233037ebb59f1b6a6cd130bcd8e04407d23015d8078319f8c807a9c57",
            ],
            [
                "9af686d9a50887d0d291f3e80c03e107be7cf504c9ad3ebba5736707502defd7",
                "7450d671550279e69ea0895a277f10e622edd6d6eec6b2d7276ac55d1ab9e5e7",
            ],
        ],
        [
            [
                "6e661bdcd647c645e8b7367f8320fe01641656836207155726589c7c78bf5832",
                "8ac31e9a5337452968183097e40563151965138ec1b7dafd9e7094b775c8b5c8",
            ],
            [
                "254735476eb0a3136ecc223ace5960f14b676547cf498abbb47b6fe693860924",
                "52e839a33c8d84589a7ffec047869ea9d55425b615c3ca0571db8991fffd3f40",
            ],
        ],
    ],
    [
        [
            [
                "161ab2e5f585de6da11427b447ed36476ed3f5147a1124df3fa0deb784c1a072",
                "b881a24015297d24578b14362056ba2df728960ec0c96722c2f8a1a31fa49c47",
            ],
            [
                "f54a165f5bbbe253afb9687655837f39911588c1d595329ed6255aea14fcee0c",
                "8e3b2218d2051477806e0cea65b72d112f853dd5ab64d8f90029d4a1a801a528",
            ],
        ],
        [
            [
                "53be319f8d02937391ddeb038c5b2141f07c512c5a4fe5549f94c72be8986abc",
                "115a2842d1377f7f487075f08603b7611aedb5534136ac477fe6d2de624e93fa",
            ],
            [
                "92b9a447c92c02cb3691ce330b66809143617a15180fd0f5fcb07d11c8a0c977",
                "6626bb735da78e7e7e2a949b79c262ac3498414c9786ecc44b2189032a9fe21f",
            ],
        ],
        [
            [
                "0bb4c55238aa500f57e28f7ac2148bdd9b5d30c3fcfaeef0e7090830f0f7d3f0",
                "cea4032ff41e01d49ccdd230c0da83ce613b20bc6264e84d11b4ae20d1d0d474",
            ],
            [
                "92b9a447c92c02cb3691ce330b66809143617a15180fd0f5fcb07d11c8a0c977",
                "6626bb735da78e7e7e2a949b79c262ac3498414c9786ecc44b2189032a9fe21f",
            ],
        ],
        [
            [
                "07d69b0788f991bca33616e5bb010bba8a02aba2f766e2aade5392da904b88bf",
                "c0e38117aa766a19d7a0aeafe35e5083e8cd8ba4809371965057f706426fdffe",
            ],
            [
                "b04c92749a7b090c89f0461213e30f3bebe0482f07b9f543a91dfb8eeadd2c9c",
                "1cf039b7ed5679fbebe418baa80ea7da6ab91cbbbf11d164640508a7db7baffc",
            ],
        ],
        [
            [
                "ad888a6f0e35ad62ace6b25f125aadca445dec122c7f4c93fba11273ca0e06e6",
                "ceacff11419a34b4113a569af0880b4d112f9513e5b9a6cdd228494af937c224",
            ],
            [
                "0e70556c62a1319afed89a66ec1783edc6908fc2f6dcd03ccd22e214e023fa4c",
                "c1938b3d7201e865592f706aa0310a7e5c5f756bce3e98828244e5d99397d672",
            ],
        ],
        [
            [
                "c3a65a596fb588fffa7be0b46899f6190b9e952d165d8d7ad60fb1639b5f8bb4",
                "c545353ae392fa5b9591586c39ea2eed15e21091f6fb02f36577b9e7d9c30442",
            ],
            [
                "c86e22b73cb4aeb5d699020c3453a608e98d17f8f0f80a9da785c2f4ad7c33b9",
                "5506727bca762a07c520651f4e53f2f48b39a2c4f2c5370a0bb677e09cf12bf2",
            ],
        ],
        [
            [
                "8ad30a5f94b5af494394c2b23cf4fe446c3adb549ea184afd6ba7834e65b8d4d",
                "c66d11b3425b8cc2a445c24855d7578f69638a85af1924efb1d1aa278d2852a4",
            ],
            [
                "97593615c5a512a699205e9915054d50b35835535be177a2eabafaa214555a46",
                "2f26c85827841404b98ed29bedb6e1ab529e88c55de67db1cb07c10a5780c398",
            ],
        ],
        [
            [
                "074d1a25165b56c00abed3d91fa883e4a377d2b06e4962804d82f80369dd7be7",
                "b5bb8e9ba12e6a01a549d4572d581602a3d4532c3014fc3c5ffe2b30209d0150",
            ],
            [
                "efae54e07383553c234b5ddd5e365614a307fc3102e78859ca3de57e77f00553",
                "abd22cb53191227ccb601cfa1e86cf6d146ac452097ef9a920b47a9bc5ecf6aa",
            ],
        ],
    ],
];

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
                        println!("{} {li} {perceptual} {exact} {got}", target as usize);
                    }
                    let want = RECORDED[target as usize][li][perceptual as usize][exact as usize];
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
        return;
    }
    assert!(
        diffs.is_empty(),
        "bc7e output changed for shipped levels (goldens would change):\n{}",
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
        RECORDED[IspcTarget::Avx512 as usize],
        RECORDED[IspcTarget::Avx2 as usize]
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
