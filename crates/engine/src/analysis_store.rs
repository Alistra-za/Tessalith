//! The on-disk side of [`crate::session::AnalysisCache`] (DECISIONS Note DZ): each source's (or packed image's)
//! analysis, source info and thumbnail, kept between sessions so an unchanged image is not decoded again.
//!
//! One file per entry, named by the sha256 of its key: a JSON header line, then the thumbnail's RGBA8 bytes,
//! deflated when that makes them smaller (Note HV1). The
//! key holds everything an entry's meaning depends on, and the header repeats it, so a lookup trusts only an entry
//! whose stored key equals the wanted one. Every failure is a miss: the cache is an optimisation, never an error.
use crate::analyse::{AnalyseHints, Analysis};
use crate::dds::write_atomic;
use crate::image::SourceInfo;
use crate::policy::SourceStamp;
use crate::session::{PackEntry, Thumbnail, THUMB_MAX};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// A hit on an entry older than this moves its modification time to now (Note HR7).
const TOUCH_AFTER: Duration = Duration::from_secs(24 * 3600);

/// Bump whenever decoding (`load_image_meta`), `analyse_with`, `make_thumbnail`, pack assembly or any `Analysis` field
/// changes what it produces: entries written under another value are ignored. The pin test (analysis_version.rs)
/// fails when those change for what it runs: PNG (8-bit, 16-bit, gray), 2D BC1/BC5/BC7 and single-cube DDS, two
/// packs, and the `Analysis` schema. PSD and Krita/ORA decoding are not run by it: bump by hand (Note DZ4).
pub const ANALYSIS_VERSION: u32 = 3;

const EXT: &str = "ent";

pub struct AnalysisStore {
    dir: PathBuf,
    version: u32,
}

#[derive(Serialize, Deserialize)]
struct Head {
    id: String,
    analysis: Analysis,
    info: SourceInfo,
    #[serde(default)]
    non_gray: Vec<PathBuf>,
    /// Width and height of the RGBA8 thumbnail after the header line (stored as `pack_thumb` leaves it).
    thumb: Option<(u32, u32)>,
}

/// A thumbnail's bytes as an entry holds them (Note HV1): zlib-deflated when that is smaller, else as they are. The
/// two never have the same length, so a reader needs no flag.
fn pack_thumb(rgba: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    use std::io::Write as _;
    // Fast: level 6 saves 2% more of the raw size for several times the work (measured on 600 real entries).
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    match z.write_all(rgba).and_then(|()| z.finish()) {
        Ok(packed) if packed.len() < rgba.len() => packed.into(),
        _ => rgba.into(),
    }
}

/// The `len` thumbnail bytes behind `stored`: `stored` itself when it has that length (not deflated, or written
/// before Note HV), else its inflated bytes. `None` unless `stored` is one whole zlib stream, with its checksum and
/// nothing after it, that inflates to exactly `len` bytes. The caller bounds `len`; no more than that is reserved.
fn unpack_thumb(stored: &[u8], len: usize) -> Option<Vec<u8>> {
    if stored.len() == len {
        return Some(stored.to_vec());
    }
    // `decompress_vec` fills the reserved room only, so a longer stream does not end and is a miss.
    let mut out = Vec::with_capacity(len);
    let mut z = flate2::Decompress::new(true);
    let end = z
        .decompress_vec(stored, &mut out, flate2::FlushDecompress::Finish)
        .ok()?;
    let whole = end == flate2::Status::StreamEnd && z.total_in() == stored.len() as u64;
    (whole && out.len() == len).then_some(out)
}

impl AnalysisStore {
    pub fn new(dir: PathBuf) -> AnalysisStore {
        AnalysisStore {
            dir,
            version: ANALYSIS_VERSION,
        }
    }
    /// A store that behaves as another analysis version (tests).
    #[doc(hidden)]
    pub fn with_version(mut self, version: u32) -> AnalysisStore {
        self.version = version;
        self
    }

    fn id(&self, hints: AnalyseHints, what: &str) -> String {
        format!("{}|{hints:?}|{what}", self.version)
    }
    fn file_id(&self, path: &Path, stamp: SourceStamp, hints: AnalyseHints) -> String {
        // Absolute, so a relative path never matches another folder's file after a cwd change (Note DZ9).
        let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        let what = format!("file|{}|{}|{}", abs.display(), stamp.size, stamp.mtime_ns);
        self.id(hints, &what)
    }
    fn pack_id(&self, key: &str, hints: AnalyseHints) -> String {
        self.id(hints, &format!("pack|{key}"))
    }
    fn path_of(&self, id: &str) -> PathBuf {
        self.dir
            .join(format!("{}.{EXT}", hex::encode(Sha256::digest(id))))
    }

    /// The entry stored under `id`; a missing, unreadable, corrupt or mismatched file is `None`.
    fn get(&self, id: &str) -> Option<(Head, Option<Arc<Thumbnail>>)> {
        let path = self.path_of(id);
        let mut file = std::fs::File::open(&path).ok()?;
        let modified = file.metadata().and_then(|m| m.modified()).ok();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut file, &mut bytes).ok()?;
        drop(file);
        let nl = bytes.iter().position(|&b| b == b'\n')?;
        let head: Head = serde_json::from_slice(&bytes[..nl]).ok()?;
        if head.id != id {
            return None;
        }
        let stored = &bytes[nl + 1..];
        let thumb = match head.thumb {
            // No thumbnail is larger than `THUMB_MAX` a side: a header that says so is damaged, and its size is not
            // reserved.
            Some((width, height)) if width.max(height) <= THUMB_MAX => {
                let len = width as usize * height as usize * 4;
                Some(Arc::new(Thumbnail {
                    width,
                    height,
                    rgba: unpack_thumb(stored, len)?,
                }))
            }
            None if stored.is_empty() => None,
            _ => return None,
        };
        if modified.is_some_and(|m| m.elapsed().is_ok_and(|age| age > TOUCH_AFTER)) {
            // A failed touch (read-only entry, share lock) is still a hit.
            let _ = std::fs::File::options()
                .write(true)
                .open(&path)
                .and_then(|f| f.set_modified(SystemTime::now()));
        }
        Some((head, thumb))
    }
    /// Store an entry; errors are ignored.
    fn put(&self, head: Head, thumb: Option<&Thumbnail>) {
        let Ok(mut bytes) = serde_json::to_vec(&head) else {
            return;
        };
        // serde_json writes a non-finite float as `null`, which does not read back: such an analysis is not stored.
        match serde_json::from_slice::<Head>(&bytes) {
            Ok(back) if back.analysis == head.analysis => {}
            _ => return,
        }
        bytes.push(b'\n');
        if let Some(t) = thumb {
            bytes.extend_from_slice(&pack_thumb(&t.rgba));
        }
        let _ = write_atomic(&self.path_of(&head.id), &bytes);
    }

    pub(crate) fn get_file(
        &self,
        path: &Path,
        stamp: SourceStamp,
        hints: AnalyseHints,
    ) -> Option<(Analysis, SourceInfo, Option<Arc<Thumbnail>>)> {
        let (h, t) = self.get(&self.file_id(path, stamp, hints))?;
        Some((h.analysis, h.info, t))
    }
    pub(crate) fn put_file(
        &self,
        path: &Path,
        stamp: SourceStamp,
        hints: AnalyseHints,
        analysis: &Analysis,
        info: SourceInfo,
        thumb: Option<&Thumbnail>,
    ) {
        let head = Head {
            id: self.file_id(path, stamp, hints),
            analysis: analysis.clone(),
            info,
            non_gray: Vec::new(),
            thumb: thumb.map(|t| (t.width, t.height)),
        };
        self.put(head, thumb);
    }
    pub(crate) fn get_pack(&self, key: &str, hints: AnalyseHints) -> Option<PackEntry> {
        let (h, thumb) = self.get(&self.pack_id(key, hints))?;
        Some(PackEntry {
            analysis: h.analysis,
            info: h.info,
            non_gray: h.non_gray,
            thumb,
            hints,
        })
    }
    pub(crate) fn put_pack(&self, key: &str, hints: AnalyseHints, e: &PackEntry) {
        let head = Head {
            id: self.pack_id(key, hints),
            analysis: e.analysis.clone(),
            info: e.info,
            non_gray: e.non_gray.clone(),
            thumb: e.thumb.as_ref().map(|t| (t.width, t.height)),
        };
        self.put(head, e.thumb.as_deref());
    }

    /// Delete the oldest entries (by modification time) until the folder holds at most `max_bytes`, and any temp file
    /// a crashed write left behind more than a day ago.
    // An entry's time is when it was written or, once a day, last read (Note HR7).
    pub fn prune(&self, max_bytes: u64) {
        let Ok(dir) = std::fs::read_dir(&self.dir) else {
            return;
        };
        let mut all: Vec<_> = dir
            .filter_map(|e| {
                let e = e.ok()?;
                let path = e.path();
                let m = e.metadata().ok()?;
                if path.extension()? == "tmp" {
                    let day = std::time::Duration::from_secs(24 * 3600);
                    if m.modified().ok()?.elapsed().is_ok_and(|age| age > day) {
                        let _ = std::fs::remove_file(&path);
                    }
                    return None;
                }
                (path.extension()? == EXT).then_some(())?;
                Some((m.modified().ok()?, m.len(), path))
            })
            .collect();
        let mut total: u64 = all.iter().map(|e| e.1).sum();
        all.sort();
        for (_, len, path) in all {
            if total <= max_bytes {
                break;
            }
            if std::fs::remove_file(path).is_ok() {
                total -= len;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyse::analyse;
    use crate::image::Rgba32F;

    fn sample() -> (Analysis, SourceInfo) {
        let info = SourceInfo {
            bit_depth: 8,
            channels: 4,
            has_alpha: true,
            is_float: false,
        };
        (
            analyse(&Rgba32F::new(2, 2, [0.5, 0.25, 1.0, 1.0]), &info),
            info,
        )
    }
    const STAMP: SourceStamp = SourceStamp {
        size: 7,
        mtime_ns: 9,
    };

    #[test]
    fn a_stored_entry_reads_back_and_another_version_misses() {
        let dir = tempfile::tempdir().unwrap();
        let (a, info) = sample();
        let thumb = Thumbnail {
            width: 1,
            height: 2,
            rgba: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        let (p, h) = (Path::new("a.png"), AnalyseHints::default());
        let s = AnalysisStore::new(dir.path().into());
        s.put_file(p, STAMP, h, &a, info, Some(&thumb));
        let (a2, i2, t2) = s.get_file(p, STAMP, h).unwrap();
        assert_eq!((a2, i2, t2.as_deref()), (a.clone(), info, Some(&thumb)));
        let other = AnalysisStore::new(dir.path().into()).with_version(ANALYSIS_VERSION + 1);
        assert!(other.get_file(p, STAMP, h).is_none());
    }

    /// 64 x 64 texels of two ramps: bytes that repeat, as a real thumbnail's do.
    fn ramp_thumb() -> Thumbnail {
        Thumbnail {
            width: 64,
            height: 64,
            rgba: (0..64 * 64)
                .flat_map(|i| [(i % 64) as u8, (i / 64) as u8, 128, 255])
                .collect(),
        }
    }
    /// The one entry file in `dir`, and where its thumbnail bytes start.
    fn only_entry(dir: &Path) -> (PathBuf, Vec<u8>, usize) {
        let path = std::fs::read_dir(dir)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let bytes = std::fs::read(&path).unwrap();
        let at = bytes.iter().position(|&b| b == b'\n').unwrap() + 1;
        (path, bytes, at)
    }
    fn zlib(data: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut z = flate2::write::ZlibEncoder::new(vec![], flate2::Compression::fast());
        z.write_all(data).unwrap();
        z.finish().unwrap()
    }

    /// Note HV1: the thumbnail is stored deflated and reads back byte for byte.
    #[test]
    fn a_thumbnail_is_stored_compressed_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let (a, info) = sample();
        let thumb = ramp_thumb();
        let (p, h) = (Path::new("a.png"), AnalyseHints::default());
        let s = AnalysisStore::new(dir.path().into());
        s.put_file(p, STAMP, h, &a, info, Some(&thumb));
        let (_, bytes, at) = only_entry(dir.path());
        let stored = bytes.len() - at;
        assert!(
            stored < thumb.rgba.len() / 2,
            "{stored} bytes stored for a thumbnail of {}",
            thumb.rgba.len()
        );
        let (_, _, t) = s.get_file(p, STAMP, h).unwrap();
        assert_eq!(t.as_deref(), Some(&thumb));
    }

    /// HV1: an entry written before thumbnails were compressed holds the bytes as they are, and still reads.
    #[test]
    fn an_entry_with_an_uncompressed_thumbnail_still_reads() {
        let dir = tempfile::tempdir().unwrap();
        let (a, info) = sample();
        let thumb = ramp_thumb();
        let (p, h) = (Path::new("a.png"), AnalyseHints::default());
        let s = AnalysisStore::new(dir.path().into());
        s.put_file(p, STAMP, h, &a, info, Some(&thumb));
        let (path, mut bytes, at) = only_entry(dir.path());
        bytes.truncate(at);
        bytes.extend_from_slice(&thumb.rgba);
        std::fs::write(&path, bytes).unwrap();
        let (_, _, t) = s.get_file(p, STAMP, h).unwrap();
        assert_eq!(t.as_deref(), Some(&thumb));
    }

    /// HV1: a thumbnail deflate cannot shrink is stored as it is, so the length alone says which form an entry holds.
    #[test]
    fn a_thumbnail_that_does_not_shrink_is_stored_as_it_is() {
        let dir = tempfile::tempdir().unwrap();
        let (a, info) = sample();
        let thumb = Thumbnail {
            width: 1,
            height: 2,
            rgba: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        assert!(zlib(&thumb.rgba).len() >= thumb.rgba.len());
        let (p, h) = (Path::new("a.png"), AnalyseHints::default());
        AnalysisStore::new(dir.path().into()).put_file(p, STAMP, h, &a, info, Some(&thumb));
        let (_, bytes, at) = only_entry(dir.path());
        assert_eq!(bytes[at..], thumb.rgba);
    }

    /// HV1: what `pack_thumb` returns is the input itself or strictly shorter, so no deflated thumbnail has the
    /// length of its texels, which is all a reader goes by. The inputs are noise followed by a growing run of zeros,
    /// so their deflated length passes their own length; at least one must land exactly on it.
    #[test]
    fn a_packed_thumbnail_never_has_the_length_of_its_texels() {
        let mut x = 1u32;
        let mut data: Vec<u8> = (0..64)
            .map(|_| {
                x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (x >> 24) as u8
            })
            .collect();
        let mut ties = 0;
        for _ in 0..200 {
            data.push(0);
            let packed = pack_thumb(&data);
            assert!(
                *packed == *data || packed.len() < data.len(),
                "{} bytes packed to {}",
                data.len(),
                packed.len()
            );
            assert_eq!(unpack_thumb(&packed, data.len()).as_deref(), Some(&*data));
            ties += usize::from(zlib(&data).len() == data.len());
        }
        assert!(
            ties > 0,
            "no input deflates to its own length: this test pins nothing"
        );
    }

    /// A compressed thumbnail that is cut, changed, or longer than its header says is a miss, never a wrong picture.
    #[test]
    fn a_damaged_compressed_thumbnail_is_a_miss() {
        let dir = tempfile::tempdir().unwrap();
        let (a, info) = sample();
        let thumb = ramp_thumb();
        let (p, h) = (Path::new("a.png"), AnalyseHints::default());
        let s = AnalysisStore::new(dir.path().into());
        s.put_file(p, STAMP, h, &a, info, Some(&thumb));
        let (path, good, at) = only_entry(dir.path());
        assert!(s.get_file(p, STAMP, h).is_some());

        let mut cut = good.clone();
        cut.truncate(good.len() - 5);
        let mut changed = good.clone();
        let mid = at + (good.len() - at) / 2;
        changed[mid] ^= 0x55;
        let mut longer = good[..at].to_vec();
        longer.extend(zlib(&[thumb.rgba.clone(), thumb.rgba.clone()].concat()));
        let mut shorter = good[..at].to_vec();
        shorter.extend(zlib(&thumb.rgba[..thumb.rgba.len() / 2]));
        // The last four bytes are the stream's checksum: every texel comes out without them.
        let mut no_checksum = good.clone();
        no_checksum.truncate(good.len() - 4);
        let mut bad_checksum = good.clone();
        *bad_checksum.last_mut().unwrap() ^= 1;
        let mut trailing = good.clone();
        trailing.push(0);
        for (name, bytes) in [
            ("cut", cut),
            ("changed", changed),
            ("longer", longer),
            ("shorter", shorter),
            ("no checksum", no_checksum),
            ("bad checksum", bad_checksum),
            ("trailing", trailing),
        ] {
            std::fs::write(&path, bytes).unwrap();
            assert!(s.get_file(p, STAMP, h).is_none(), "{name}");
        }
    }

    #[test]
    fn a_non_finite_analysis_is_not_stored() {
        let dir = tempfile::tempdir().unwrap();
        let (mut a, info) = sample();
        a.corner_err = f32::INFINITY;
        let (p, h) = (Path::new("a.png"), AnalyseHints::default());
        let s = AnalysisStore::new(dir.path().into());
        s.put_file(p, STAMP, h, &a, info, None);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        assert!(s.get_file(p, STAMP, h).is_none());
    }

    #[test]
    fn prune_keeps_the_newest_entries() {
        let dir = tempfile::tempdir().unwrap();
        let t0 = std::time::SystemTime::now();
        for (i, name) in ["a", "b", "c"].iter().enumerate() {
            let f = std::fs::File::create(dir.path().join(format!("{name}.{EXT}"))).unwrap();
            f.set_len(100).unwrap();
            f.set_modified(t0 + std::time::Duration::from_secs(i as u64))
                .unwrap();
        }
        AnalysisStore::new(dir.path().into()).prune(250);
        assert!(!dir.path().join("a.ent").exists());
        assert!(dir.path().join("b.ent").exists() && dir.path().join("c.ent").exists());
    }

    fn age(path: &Path, by: Duration) {
        let f = std::fs::File::options().write(true).open(path).unwrap();
        f.set_modified(SystemTime::now() - by).unwrap();
    }
    const DAY: Duration = Duration::from_secs(24 * 3600);

    /// Note HR7: a hit on an entry older than a day makes it the newest, so the prune takes the unused one.
    #[test]
    fn a_hit_on_an_old_entry_survives_the_prune() {
        let dir = tempfile::tempdir().unwrap();
        let (a, info) = sample();
        let h = AnalyseHints::default();
        let s = AnalysisStore::new(dir.path().into());
        let (used, unused) = (Path::new("used.png"), Path::new("unused.png"));
        s.put_file(used, STAMP, h, &a, info, None);
        s.put_file(unused, STAMP, h, &a, info, None);
        let (pu, pn) = (
            s.path_of(&s.file_id(used, STAMP, h)),
            s.path_of(&s.file_id(unused, STAMP, h)),
        );
        age(&pu, 3 * DAY);
        age(&pn, 2 * DAY);
        assert!(s.get_file(used, STAMP, h).is_some());
        // The budget is the used entry's size: the test relies on it fitting alone and not fitting with the other.
        s.prune(std::fs::metadata(&pu).unwrap().len());
        assert!(pu.exists() && !pn.exists());
    }

    /// Note HR7: a hit within the day writes nothing (guard: pins the day threshold).
    #[test]
    fn a_recent_hit_leaves_the_time_alone() {
        let dir = tempfile::tempdir().unwrap();
        let (a, info) = sample();
        let h = AnalyseHints::default();
        let s = AnalysisStore::new(dir.path().into());
        let p = Path::new("a.png");
        s.put_file(p, STAMP, h, &a, info, None);
        let path = s.path_of(&s.file_id(p, STAMP, h));
        age(&path, DAY / 24);
        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert!(s.get_file(p, STAMP, h).is_some());
        assert_eq!(
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            before
        );
    }

    /// Note HR7: an entry that cannot be written is still a hit (guard: pins existing behaviour).
    #[test]
    fn a_read_only_old_entry_is_still_a_hit() {
        let dir = tempfile::tempdir().unwrap();
        let (a, info) = sample();
        let h = AnalyseHints::default();
        let s = AnalysisStore::new(dir.path().into());
        let p = Path::new("a.png");
        s.put_file(p, STAMP, h, &a, info, None);
        let path = s.path_of(&s.file_id(p, STAMP, h));
        age(&path, 3 * DAY);
        let set_ro = |ro: bool| {
            let mut perm = std::fs::metadata(&path).unwrap().permissions();
            perm.set_readonly(ro);
            std::fs::set_permissions(&path, perm).unwrap();
        };
        set_ro(true);
        let hit = s.get_file(p, STAMP, h).is_some();
        set_ro(false);
        assert!(hit);
    }

    #[test]
    fn prune_removes_old_stray_temp_files_only() {
        let dir = tempfile::tempdir().unwrap();
        let day = std::time::Duration::from_secs(24 * 3600);
        for (name, age) in [(".sdc-1-2-3.tmp", 2 * day), (".sdc-1-2-4.tmp", day / 24)] {
            let f = std::fs::File::create(dir.path().join(name)).unwrap();
            f.set_modified(std::time::SystemTime::now() - age).unwrap();
        }
        AnalysisStore::new(dir.path().into()).prune(u64::MAX);
        assert!(!dir.path().join(".sdc-1-2-3.tmp").exists());
        assert!(dir.path().join(".sdc-1-2-4.tmp").exists());
    }

    #[test]
    fn an_overflowing_thumbnail_size_is_a_miss() {
        let dir = tempfile::tempdir().unwrap();
        let (a, info) = sample();
        let (p, h) = (Path::new("a.png"), AnalyseHints::default());
        let s = AnalysisStore::new(dir.path().into());
        let head = Head {
            id: s.file_id(p, STAMP, h),
            analysis: a,
            info,
            non_gray: Vec::new(),
            thumb: Some((u32::MAX, u32::MAX)),
        };
        s.put(head, None);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        assert!(s.get_file(p, STAMP, h).is_none());
    }

    /// A thumbnail of the largest size reads; a header that names a larger one is a miss, whatever follows it, so a
    /// damaged header never sizes the buffer its thumbnail is inflated into.
    #[test]
    fn a_thumbnail_larger_than_any_made_is_a_miss() {
        let dir = tempfile::tempdir().unwrap();
        let (a, info) = sample();
        let (p, h) = (Path::new("a.png"), AnalyseHints::default());
        let s = AnalysisStore::new(dir.path().into());
        for (width, hit) in [(THUMB_MAX, true), (THUMB_MAX + 1, false)] {
            let thumb = Thumbnail {
                width,
                height: 1,
                rgba: vec![7; width as usize * 4],
            };
            s.put_file(p, STAMP, h, &a, info, Some(&thumb));
            assert_eq!(s.get_file(p, STAMP, h).is_some(), hit, "{width} wide");
        }
    }

    #[test]
    fn a_relative_path_keys_as_its_absolute_one() {
        let s = AnalysisStore::new(PathBuf::new());
        let h = AnalyseHints::default();
        let abs = std::path::absolute("x/a.png").unwrap();
        assert_eq!(
            s.file_id(Path::new("x/a.png"), STAMP, h),
            s.file_id(&abs, STAMP, h)
        );
    }
}
