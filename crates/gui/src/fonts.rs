//! Interface fonts: the OS's own by family name, then an embedded floor; missing characters come from the system on demand (Note JM).
//!
//! The order ([`resolve`]): the proportional family takes the first font found of the OS's named list, or the OS's
//! generic `sans-serif` answer, generic first where [`OsFonts::generic_first`] says so (Linux); then every extra of
//! the table that is found. The monospace family does the same with its own list and `monospace`, and falls back to
//! the proportional system font. Where nothing is found, the embedded floor (Ubuntu Light, Hack, egui-icons) is used.
//! On Windows that is Segoe UI with Segoe UI Symbol and Segoe UI Emoji, and Consolas with Segoe UI Symbol.
use eframe::egui;
use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

/// The system's fonts asked by family name, through the OS's own font list (DirectWrite, fontconfig, CoreText).
pub(crate) struct SystemLookup {
    collection: fontique::Collection,
    sources: fontique::SourceCache,
}

impl SystemLookup {
    /// Enumerates the system fonts on the calling thread.
    pub fn new() -> Self {
        SystemLookup {
            collection: fontique::Collection::new(fontique::CollectionOptions {
                shared: false,
                system_fonts: true,
            }),
            sources: fontique::SourceCache::default(),
        }
    }

    /// The family's regular face (normal width, upright, weight 400). A variable font draws its default instance.
    fn face(&mut self, family: &fontique::FamilyInfo) -> Option<Found> {
        let font = family.match_font(
            fontique::FontWidth::NORMAL,
            fontique::FontStyle::Normal,
            fontique::FontWeight::NORMAL,
            false,
        )?;
        let blob = font.load(Some(&mut self.sources))?;
        Some(Found {
            family: family.name().to_owned(),
            data: egui::FontData::from_blob(blob.into_raw_parts().0, font.index()),
        })
    }
}

impl FontLookup for SystemLookup {
    fn family(&mut self, name: &str) -> Option<Found> {
        let family = self.collection.family_by_name(name)?;
        self.face(&family)
    }

    /// The first family of the OS's generic list. On Windows the lists are fixed names filtered by presence (Arial,
    /// Consolas), so without Consolas the monospace answer is `None`.
    fn generic(&mut self, generic: Generic) -> Option<Found> {
        let generic = match generic {
            Generic::SansSerif => fontique::GenericFamily::SansSerif,
            Generic::Monospace => fontique::GenericFamily::Monospace,
        };
        let id = self.collection.generic_families(generic).next()?;
        let family = self.collection.family(id)?;
        self.face(&family)
    }
}

/// `f`'s definitions, or the embedded floor with `lookup_failed` when it panics (DirectWrite that cannot start). The
/// panic is logged as one line, not as a crash (`crash_log::catch_expected`).
pub fn guarded(
    f: impl FnOnce() -> (egui::FontDefinitions, FontChoice) + std::panic::UnwindSafe,
) -> (egui::FontDefinitions, FontChoice) {
    crate::crash_log::catch_expected("font lookup failed", f).unwrap_or_else(|_| {
        let (defs, choice) = embedded();
        (
            defs,
            FontChoice {
                lookup_failed: true,
                ..choice
            },
        )
    })
}

/// This OS's fonts, looked up once per process; a clone shares the font data.
fn system_definitions() -> (egui::FontDefinitions, FontChoice) {
    static DEFS: OnceLock<(egui::FontDefinitions, FontChoice)> = OnceLock::new();
    DEFS.get_or_init(|| {
        // Starts the provider's enumeration thread first, so it overlaps the lookup's own enumeration.
        let _ = system_provider();
        guarded(|| resolve(&mut SystemLookup::new(), os_fonts()))
    })
    .clone()
}

/// Whether `SDC_FONT`'s value forces the embedded floor: only exactly `embedded` does. A test seam; builds without
/// `test-hooks` never read the variable.
#[cfg(feature = "test-hooks")]
pub fn floor_forced(value: Option<&std::ffi::OsStr>) -> bool {
    value.is_some_and(|v| v == "embedded")
}

/// Installs this OS's interface fonts (or the embedded floor), then the one system font provider for characters they
/// lack (CJK, Arabic, …), and pins egui 0.36.2's look in both themes (`look`). The providers are replaced, not added
/// to, so eframe's own or a second call's never stacks (Note JM). Never fails: the `Result` keeps every
/// `with_context_setup` call's `.expect("system fonts")` compiling. The choice says whether a floor font is used.
pub fn install_system_fonts(ctx: &egui::Context) -> Result<FontChoice, std::convert::Infallible> {
    // The only read of `SDC_FONT`: compiled out of a build without `test-hooks`.
    #[cfg(feature = "test-hooks")]
    let forced = floor_forced(std::env::var_os("SDC_FONT").as_deref());
    #[cfg(not(feature = "test-hooks"))]
    let forced = false;
    let (defs, choice) = if forced {
        embedded()
    } else {
        system_definitions()
    };
    ctx.set_fonts(defs);
    ctx.set_font_providers(vec![system_provider()]);
    ctx.all_styles_mut(crate::look::pin_036_style);
    Ok(choice)
}

/// The process-wide system font provider: every context (the app's, each test harness's) shares one, so the system
/// fonts are enumerated once, on one background thread, and memory-mapped only when a character needs them.
pub fn system_provider() -> Arc<dyn egui::FontProvider> {
    static PROVIDER: OnceLock<Arc<dyn egui::FontProvider>> = OnceLock::new();
    PROVIDER
        .get_or_init(|| {
            Arc::new(Guarded::new(
                egui_system_font_provider::SystemFontProvider::new(),
            ))
        })
        .clone()
}

/// A font provider that answers `None` when its inner provider panics, and from then on: a failed enumeration thread
/// makes `SystemFontProvider` panic at every later lookup, and a missing glyph is better than a crash. After any caught
/// panic, `fonts_for_family` answers nothing too. Each caught panic is one crash-log line (`crash_log::catch_expected`).
/// It also drops every face the inner provider answers that does not parse (`parses`, rule 0), which epaint would panic
/// on; a dropped face is not a provider failure, so later lookups still reach the inner provider.
pub struct Guarded<P: egui::FontProvider> {
    inner: P,
    failed: AtomicBool,
}

impl<P: egui::FontProvider> Guarded<P> {
    pub fn new(inner: P) -> Self {
        Guarded {
            inner,
            failed: AtomicBool::new(false),
        }
    }

    /// `ask`'s answer, or `None` if it panics now or the inner provider panicked before.
    fn guard<T>(&self, ask: impl FnOnce(&P) -> T) -> Option<T> {
        if self.failed.load(Ordering::Relaxed) {
            return None;
        }
        let answer = crate::crash_log::catch_expected(
            "font lookup failed",
            AssertUnwindSafe(|| ask(&self.inner)),
        );
        if answer.is_err() {
            self.failed.store(true, Ordering::Relaxed);
        }
        answer.ok()
    }
}

impl<P: egui::FontProvider> egui::FontProvider for Guarded<P> {
    fn fonts_for_family(&self, family: &egui::FontFamily) -> Vec<egui::FontInsert> {
        let mut fonts = self
            .guard(|p| p.fonts_for_family(family))
            .unwrap_or_default();
        fonts.retain(|f| parses(&f.data));
        fonts
    }

    fn font_for(&self, request: &egui::FallbackRequest<'_>) -> Option<egui::FontInsert> {
        self.guard(|p| p.font_for(request))
            .flatten()
            .filter(|f| parses(&f.data))
    }
}

/// One font family the OS may have, and the key its data gets in [`egui::FontDefinitions`].
#[derive(Clone, Copy, Debug)]
pub struct Candidate {
    pub family: &'static str,
    pub key: &'static str,
}

/// A candidate whose key is its family name (Linux, macOS).
const fn named(family: &'static str) -> Candidate {
    Candidate {
        family,
        key: family,
    }
}

/// One OS's font candidates, in the order `resolve` asks for them.
pub struct OsFonts {
    /// Ask the OS's own `sans-serif` / `monospace` answer before the named lists (Linux: the desktop's choice).
    pub generic_first: bool,
    pub proportional: &'static [Candidate],
    /// Fallbacks after the proportional primary, every one that is found.
    pub proportional_extra: &'static [Candidate],
    pub monospace: &'static [Candidate],
    /// Fallbacks after the monospace primary, every one that is found.
    pub monospace_extra: &'static [Candidate],
}

pub const WINDOWS: OsFonts = OsFonts {
    generic_first: false,
    proportional: &[Candidate {
        family: "Segoe UI",
        key: "segoeui",
    }],
    proportional_extra: &[
        Candidate {
            family: "Segoe UI Symbol",
            key: "seguisym",
        },
        Candidate {
            family: "Segoe UI Emoji",
            key: "seguiemj",
        },
    ],
    monospace: &[Candidate {
        family: "Consolas",
        key: "consola",
    }],
    monospace_extra: &[Candidate {
        family: "Segoe UI Symbol",
        key: "seguisym",
    }],
};

pub const LINUX: OsFonts = OsFonts {
    generic_first: true,
    proportional: &[
        named("Noto Sans"),
        named("DejaVu Sans"),
        named("Liberation Sans"),
        named("Cantarell"),
        named("Ubuntu Sans"),
        named("Ubuntu"),
        named("Adwaita Sans"),
        named("Inter"),
        named("FreeSans"),
    ],
    proportional_extra: &[],
    monospace: &[
        named("DejaVu Sans Mono"),
        named("Liberation Mono"),
        named("Noto Sans Mono"),
        named("Ubuntu Mono"),
    ],
    monospace_extra: &[],
};

pub const MACOS: OsFonts = OsFonts {
    generic_first: false,
    proportional: &[named("SF Pro Text"), named("Helvetica Neue")],
    proportional_extra: &[],
    monospace: &[named("Menlo")],
    monospace_extra: &[],
};

/// The candidate table of the OS this build runs on.
pub fn os_fonts() -> &'static OsFonts {
    #[cfg(windows)]
    {
        &WINDOWS
    }
    #[cfg(target_os = "macos")]
    {
        &MACOS
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        &LINUX
    }
}

/// The OS's generic families: what it answers for `sans-serif` and `monospace`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Generic {
    SansSerif,
    Monospace,
}

/// A face the OS found: the family name it reported and the face's data.
#[derive(Clone, Debug)]
pub struct Found {
    pub family: String,
    pub data: egui::FontData,
}

/// How `resolve` asks the OS for fonts (the system lookup in the app, a fixed set in tests).
pub trait FontLookup {
    fn family(&mut self, name: &str) -> Option<Found>;
    fn generic(&mut self, generic: Generic) -> Option<Found>;
}

/// Where a family's primary font came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FontSource {
    /// A system font, with the family name the OS reported.
    System(String),
    /// The built-in floor (Ubuntu Light, Hack, egui-icons).
    Embedded,
}

/// What `resolve` chose for the two families.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontChoice {
    pub proportional: FontSource,
    pub monospace: FontSource,
    /// The system lookup panicked and the floor was used (set by the caller that catches it).
    pub lookup_failed: bool,
}

pub const FLOOR_NO_FONT: &str = "Tessalith found none of its usual fonts, so it uses its built-in Ubuntu Light and Hack; install Segoe UI or Arial (on Linux, Noto Sans or DejaVu Sans) for your system's look.";
pub const FLOOR_TEXT_ONLY: &str = "Tessalith found none of its usual interface fonts, so text uses its built-in Ubuntu Light while fixed-width text keeps a system font; install Segoe UI or Arial (on Linux, Noto Sans or DejaVu Sans) for your system's look.";
pub const FLOOR_LOOKUP_FAILED: &str = "Tessalith couldn't read the system's font list, so it uses its built-in Ubuntu Light and Hack; the error is in the newest crash log in %LOCALAPPDATA%\\sdc\\logs.";

impl FontChoice {
    /// The one sentence to show when a floor font is in use, `None` when both families are system fonts.
    /// Proportional `System` with monospace `Embedded` cannot happen (`resolve`, rule 3): the monospace family falls
    /// to the proportional system font before the floor (`every_choice_has_the_right_message` pins it).
    pub fn message(&self) -> Option<&'static str> {
        if self.lookup_failed {
            return Some(FLOOR_LOOKUP_FAILED);
        }
        match (&self.proportional, &self.monospace) {
            (FontSource::Embedded, FontSource::Embedded) => Some(FLOOR_NO_FONT),
            (FontSource::Embedded, FontSource::System(_)) => Some(FLOOR_TEXT_ONLY),
            (FontSource::System(_), _) => None,
        }
    }
}

/// True when epaint can load the face: `skrifa::FontRef::from_index` is the only fallible step of epaint's
/// `FontFace::new`, and a face that fails it makes `Family::new` panic.
pub(crate) fn parses(data: &egui::FontData) -> bool {
    skrifa::FontRef::from_index(data.bytes(), data.index).is_ok()
}

/// The guard on the OS's generic answer: the face draws `AZaz09`, and its `i` and `W` have different advances for a
/// proportional font, equal ones for a monospace font. A serif face is not detected. `data` must parse.
pub(crate) fn fit_for_ui(data: &egui::FontData, monospace: bool) -> bool {
    let mut defs = egui::FontDefinitions::empty();
    defs.font_data
        .insert("probe".to_owned(), Arc::new(data.clone()));
    defs.families
        .insert(egui::FontFamily::Proportional, vec!["probe".to_owned()]);
    let mut fonts = egui::epaint::text::Fonts::new(Default::default(), defs);
    let id = egui::FontId::proportional(14.0);
    if !fonts.has_glyphs(&id, "AZaz09") {
        return false;
    }
    let mut view = fonts.with_pixels_per_point(1.0);
    let same_pitch = (view.glyph_width(&id, 'i') - view.glyph_width(&id, 'W')).abs() < 0.01;
    same_pitch == monospace
}

// The floor's keys, egui's own names for these fonts.
const KEY_UBUNTU_LIGHT: &str = "Ubuntu-Light";
const KEY_HACK: &str = "Hack";
const KEY_EGUI_ICONS: &str = "egui-icons";

/// Asks a lookup at most once per family name and per generic family, and keeps only faces that parse (rule 0).
struct Asker<'a> {
    lookup: &'a mut dyn FontLookup,
    named: HashMap<&'static str, Option<Found>>,
    generic: HashMap<Generic, Option<Found>>,
}

impl Asker<'_> {
    fn family(&mut self, name: &'static str) -> Option<Found> {
        if let Some(hit) = self.named.get(name) {
            return hit.clone();
        }
        let hit = self.lookup.family(name).filter(|f| parses(&f.data));
        self.named.insert(name, hit.clone());
        hit
    }

    /// The first candidate found, with its key.
    fn first(&mut self, list: &[Candidate]) -> Option<(String, Found)> {
        list.iter()
            .find_map(|c| self.family(c.family).map(|f| (c.key.to_owned(), f)))
    }

    /// The OS's answer for `generic` under its generic key, if it parses and passes the guard.
    fn generic(&mut self, generic: Generic) -> Option<(String, Found)> {
        let monospace = generic == Generic::Monospace;
        let hit = match self.generic.get(&generic) {
            Some(hit) => hit.clone(),
            None => {
                let hit = self
                    .lookup
                    .generic(generic)
                    .filter(|f| parses(&f.data) && fit_for_ui(&f.data, monospace));
                self.generic.insert(generic, hit.clone());
                hit
            }
        };
        let key = if monospace { "monospace" } else { "sans-serif" };
        hit.map(|f| (key.to_owned(), f))
    }
}

/// Builds the font definitions from what `lookup` finds for `os`'s candidates, with the embedded floor (Ubuntu Light,
/// Hack, egui-icons) where nothing is found (Note JM). Rule 0: a face that does not parse counts as not found. Rule 1:
/// the proportional primary is the first named candidate found, then the generic `sans-serif` answer (the other way
/// round when `generic_first`). Rule 2: every extra found follows its primary, in table order. Rule 3: the monospace
/// primary comes from its own list and the `monospace` answer, with the proportional system font before the floor
/// (and, when named lists come first, before the `monospace` answer). Rules 4 and 6: the floor's data is added
/// whenever a chain names it, and each family name is asked once.
pub fn resolve(lookup: &mut dyn FontLookup, os: &OsFonts) -> (egui::FontDefinitions, FontChoice) {
    let mut ask = Asker {
        lookup,
        named: HashMap::new(),
        generic: HashMap::new(),
    };
    let mut defs = egui::FontDefinitions::empty();

    // Rule 1: the proportional primary.
    let proportional = if os.generic_first {
        ask.generic(Generic::SansSerif)
            .or_else(|| ask.first(os.proportional))
    } else {
        ask.first(os.proportional)
            .or_else(|| ask.generic(Generic::SansSerif))
    };
    // Rule 3: the monospace primary; the proportional system font comes before the floor.
    let monospace = if os.generic_first {
        ask.generic(Generic::Monospace)
            .or_else(|| ask.first(os.monospace))
            .or_else(|| proportional.clone())
    } else {
        ask.first(os.monospace)
            .or_else(|| proportional.clone())
            .or_else(|| ask.generic(Generic::Monospace))
    };

    let (proportional_keys, proportional_source) = chain(
        &proportional,
        os.proportional_extra,
        KEY_UBUNTU_LIGHT,
        [KEY_HACK, KEY_EGUI_ICONS],
        &mut ask,
        &mut defs,
    );
    let (monospace_keys, monospace_source) = chain(
        &monospace,
        os.monospace_extra,
        KEY_HACK,
        [KEY_UBUNTU_LIGHT, KEY_EGUI_ICONS],
        &mut ask,
        &mut defs,
    );

    // Rules 4 and 6: the floor's data is there whenever a chain names it.
    if proportional_source == FontSource::Embedded || monospace_source == FontSource::Embedded {
        use epaint_default_fonts::{EGUI_ICONS, HACK_REGULAR, UBUNTU_LIGHT};
        let floor = [
            (KEY_UBUNTU_LIGHT, egui::FontData::from_static(UBUNTU_LIGHT)),
            (KEY_HACK, egui::FontData::from_static(HACK_REGULAR)),
            (
                KEY_EGUI_ICONS,
                egui::FontData::from_static(EGUI_ICONS).tweak(egui::FontTweak {
                    scale: 0.90,
                    ..Default::default()
                }),
            ),
        ];
        for (key, data) in floor {
            defs.font_data.insert(key.to_owned(), Arc::new(data));
        }
    }
    defs.families
        .insert(egui::FontFamily::Proportional, proportional_keys);
    defs.families
        .insert(egui::FontFamily::Monospace, monospace_keys);
    let choice = FontChoice {
        proportional: proportional_source,
        monospace: monospace_source,
        lookup_failed: false,
    };
    (defs, choice)
}

/// One family's chain: its primary (or the floor's `floor_first`), every extra that is found in table order (rule 2),
/// then `floor_rest` after a floor primary. Inserts the data of every system font it names.
fn chain(
    primary: &Option<(String, Found)>,
    extras: &[Candidate],
    floor_first: &str,
    floor_rest: [&str; 2],
    ask: &mut Asker<'_>,
    defs: &mut egui::FontDefinitions,
) -> (Vec<String>, FontSource) {
    let mut add = |key: &str, found: &Found| {
        defs.font_data
            .entry(key.to_owned())
            .or_insert_with(|| Arc::new(found.data.clone()));
        key.to_owned()
    };
    let mut keys = Vec::new();
    let source = match primary {
        Some((key, found)) => {
            keys.push(add(key, found));
            FontSource::System(found.family.clone())
        }
        None => {
            keys.push(floor_first.to_owned());
            FontSource::Embedded
        }
    };
    for c in extras {
        if let Some(found) = ask.family(c.family) {
            keys.push(add(c.key, &found));
        }
    }
    if source == FontSource::Embedded {
        keys.extend(floor_rest.map(str::to_owned));
    }
    (keys, source)
}

/// A lookup that finds nothing.
struct NoFonts;

impl FontLookup for NoFonts {
    fn family(&mut self, _: &str) -> Option<Found> {
        None
    }
    fn generic(&mut self, _: Generic) -> Option<Found> {
        None
    }
}

/// The embedded floor alone: Ubuntu Light for text, Hack for fixed-width text, egui-icons after both.
pub fn embedded() -> (egui::FontDefinitions, FontChoice) {
    let none = OsFonts {
        generic_first: false,
        proportional: &[],
        proportional_extra: &[],
        monospace: &[],
        monospace_extra: &[],
    };
    resolve(&mut NoFonts, &none)
}

/// Every non-ASCII character in `crates/gui/src`, CJK aside; in this file only the code before its test module, without
/// this literal (`ui_symbols_cover_the_source`).
#[cfg(test)]
pub(crate) const UI_SYMBOLS: &str = "§…—→·▾×–↺✕ß−⚠🔒≥≈≡✓ⓘẞ°±Δ²α↔üïÄ✎●←‹›äÖΩω≠π▼◀▶▸✖“”🔗é⏵";

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::FontProvider as _;
    use std::sync::atomic::AtomicUsize;

    /// Installed families by name; a named hit is Hack's bytes so `Fonts::new` can parse the result, a name in `broken`
    /// answers bytes that do not parse. The generic answers carry their own bytes (Ubuntu-Light = a proportional Latin
    /// font, Hack = monospace, egui-icons = no Latin, `JUNK` = not a font).
    struct Fake {
        have: Vec<&'static str>,
        broken: Vec<&'static str>,
        sans: Option<(&'static str, &'static [u8])>,
        mono: Option<(&'static str, &'static [u8])>,
        asked: Vec<String>,
    }

    impl Fake {
        fn with(names: &[&'static str]) -> Self {
            Fake {
                have: names.to_vec(),
                broken: Vec::new(),
                sans: None,
                mono: None,
                asked: Vec::new(),
            }
        }
    }

    impl FontLookup for Fake {
        fn family(&mut self, name: &str) -> Option<Found> {
            self.asked.push(name.to_owned());
            let bytes = if self.broken.contains(&name) {
                JUNK
            } else if self.have.contains(&name) {
                HACK
            } else {
                return None;
            };
            Some(Found {
                family: name.to_owned(),
                data: egui::FontData::from_static(bytes),
            })
        }

        fn generic(&mut self, generic: Generic) -> Option<Found> {
            let (tag, answer) = match generic {
                Generic::SansSerif => ("<sans-serif>", self.sans),
                Generic::Monospace => ("<monospace>", self.mono),
            };
            self.asked.push(tag.to_owned());
            answer.map(|(family, bytes)| Found {
                family: family.to_owned(),
                data: egui::FontData::from_static(bytes),
            })
        }
    }

    use epaint_default_fonts::{EGUI_ICONS, HACK_REGULAR as HACK, UBUNTU_LIGHT as UBUNTU};
    const JUNK: &[u8] = b"this is not a font file";

    #[cfg(windows)]
    fn windows_fonts_dir() -> std::path::PathBuf {
        std::env::var_os("WINDIR")
            .map(|w| std::path::PathBuf::from(w).join("Fonts"))
            .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows\Fonts"))
    }

    /// The definitions the app built before Note JM, from `%WINDIR%\Fonts`: Segoe UI (proportional) and Consolas
    /// (monospace) with Segoe UI Symbol and Segoe UI Emoji as fallbacks. A missing optional font is skipped; no
    /// Consolas puts Segoe UI first in the monospace family. The pins below compare the new definitions with these.
    /// Panics without `segoeui.ttf`: the Windows pins need Segoe UI, as before.
    #[cfg(windows)]
    fn legacy_definitions() -> egui::FontDefinitions {
        let dir = windows_fonts_dir();
        let primary_bytes = std::fs::read(dir.join("segoeui.ttf")).expect("segoeui.ttf");
        let mut defs = egui::FontDefinitions::empty();
        defs.font_data.insert(
            "segoeui".to_owned(),
            Arc::new(egui::FontData::from_owned(primary_bytes)),
        );
        let mut proportional = vec!["segoeui".to_owned()];
        let mut monospace = Vec::new();
        for (name, file, proportional_fallback, monospace_use) in [
            ("seguisym", "seguisym.ttf", true, true),
            ("seguiemj", "seguiemj.ttf", true, false),
            ("consola", "consola.ttf", false, true),
        ] {
            let Ok(bytes) = std::fs::read(dir.join(file)) else {
                continue;
            };
            defs.font_data
                .insert(name.to_owned(), Arc::new(egui::FontData::from_owned(bytes)));
            if proportional_fallback {
                proportional.push(name.to_owned());
            }
            if monospace_use {
                if name == "consola" {
                    monospace.insert(0, name.to_owned());
                } else {
                    monospace.push(name.to_owned());
                }
            }
        }
        if monospace.is_empty() || monospace[0] != "consola" {
            monospace.insert(0, "segoeui".to_owned());
        }
        defs.families
            .insert(egui::FontFamily::Proportional, proportional);
        defs.families.insert(egui::FontFamily::Monospace, monospace);
        defs
    }

    #[cfg(windows)]
    #[test]
    fn segoe_ui_is_the_same_file_as_before() {
        let (defs, choice) = system_definitions();
        assert_eq!(defs.families[&egui::FontFamily::Proportional][0], "segoeui");
        assert_eq!(choice.proportional, FontSource::System("Segoe UI".into()));
        for (key, file) in [
            ("segoeui", "segoeui.ttf"),
            ("seguisym", "seguisym.ttf"),
            ("seguiemj", "seguiemj.ttf"),
            ("consola", "consola.ttf"),
        ] {
            let Ok(want) = std::fs::read(windows_fonts_dir().join(file)) else {
                continue;
            };
            let got = &defs.font_data[key];
            assert_eq!(got.index, 0, "{key}");
            assert!(got.bytes() == want.as_slice(), "{key} is not {file}");
        }
        assert_eq!(choice.message(), None);
    }

    /// The plan's width pin: today's definitions and the new ones lay text out to the same width.
    #[cfg(windows)]
    #[test]
    fn text_is_as_wide_as_before() {
        let sample = "Cutout + Blend · Coat normal / Fuzz (slot 7) ↺ ▾ ⚠ 🔒 — ×";
        let width = |defs: egui::FontDefinitions, id: egui::FontId| {
            let mut f = egui::epaint::text::Fonts::new(Default::default(), defs);
            f.with_pixels_per_point(1.0)
                .layout_no_wrap(sample.into(), id, egui::Color32::WHITE)
                .size()
                .x
        };
        for id in [
            egui::FontId::proportional(14.0),
            egui::FontId::monospace(13.0),
        ] {
            assert_eq!(
                width(system_definitions().0, id.clone()),
                width(legacy_definitions(), id)
            );
        }
    }

    /// Each `UI_SYMBOLS` character with the key of the configured font that draws it in `family`, `None` when no
    /// configured font has it (the system provider is not asked).
    #[cfg(windows)]
    fn symbol_owners(
        defs: egui::FontDefinitions,
        family: &egui::FontFamily,
    ) -> Vec<(char, Option<String>)> {
        let mut f = egui::epaint::text::Fonts::new(Default::default(), defs);
        let mut v = f.with_pixels_per_point(1.0);
        let map = v.characters(family).clone();
        UI_SYMBOLS
            .chars()
            .map(|c| (c, map.get(&c).and_then(|n| n.first().cloned())))
            .collect()
    }

    #[cfg(windows)]
    #[test]
    fn ui_symbols_resolve_to_the_same_fonts_as_before() {
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            assert_eq!(
                symbol_owners(system_definitions().0, &family),
                symbol_owners(legacy_definitions(), &family),
                "{family:?}"
            );
        }
    }

    /// Note JM3: on Windows no UI string reaches the system provider, because every UI symbol has a configured font
    /// in both families. A new UI character none of them draws fails here, not silently through the provider.
    /// Needs this machine's Segoe UI, Segoe UI Symbol and Consolas to draw every UI symbol; a failure on another
    /// machine names the characters its fonts lack.
    #[cfg(windows)]
    #[test]
    fn every_ui_symbol_has_a_configured_font_on_windows() {
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            let missing: String = symbol_owners(system_definitions().0, &family)
                .into_iter()
                .filter(|(_, owner)| owner.is_none())
                .map(|(c, _)| c)
                .collect();
            assert!(
                missing.is_empty(),
                "{family:?}: no configured font draws {missing}"
            );
        }
    }

    /// Review Focus 2: a lookup that panics (DirectWrite cannot start) still gives fonts.
    #[test]
    fn a_lookup_that_panics_falls_back_to_the_floor() {
        let (defs, choice) = guarded(|| panic!("no DirectWrite"));
        assert_eq!(choice.proportional, FontSource::Embedded);
        assert_eq!(choice.message(), Some(FLOOR_LOOKUP_FAILED));
        assert_eq!(
            defs.families[&egui::FontFamily::Proportional][0],
            "Ubuntu-Light"
        );
    }

    #[test]
    fn install_reports_the_choice() {
        let ctx = egui::Context::default();
        let Ok(choice) = install_system_fonts(&ctx);
        assert_eq!(choice, system_definitions().1);
    }

    /// Plan 1's pinned 0.36.2 look is still applied by the rewritten installer, for both themes. `pin_036_style` only
    /// assigns fixed values, so applying it again changes nothing; without the line the default 0.37 style differs
    /// (`button_padding`, corner radii).
    #[test]
    fn install_still_applies_the_pinned_look() {
        let ctx = egui::Context::default();
        let Ok(_) = install_system_fonts(&ctx);
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            let mut want = (*ctx.style_of(theme)).clone();
            crate::look::pin_036_style(&mut want);
            assert_eq!(
                *ctx.style_of(theme),
                want,
                "{theme:?}: pin_036_style not applied"
            );
        }
    }

    #[test]
    fn the_generic_guard_wants_latin_and_the_right_pitch() {
        let d = |b: &'static [u8]| egui::FontData::from_static(b);
        assert!(fit_for_ui(&d(UBUNTU), false));
        assert!(
            !fit_for_ui(&d(HACK), false),
            "a monospace font is not a proportional UI font"
        );
        assert!(fit_for_ui(&d(HACK), true));
        assert!(
            !fit_for_ui(&d(UBUNTU), true),
            "a proportional font is not a monospace font"
        );
        assert!(!fit_for_ui(&d(EGUI_ICONS), false), "no Latin letters");
        assert!(parses(&d(UBUNTU)) && parses(&d(HACK)) && !parses(&d(JUNK)));
    }

    /// Review round 1, ruling 1: a broken file is skipped per candidate; the next candidate is used, not the floor.
    #[test]
    fn a_broken_font_file_is_skipped() {
        // Linux: the generic answer and the first named family are broken.
        let mut f = Fake {
            broken: vec!["Noto Sans"],
            sans: Some(("Broken", JUNK)),
            mono: Some(("Broken Mono", JUNK)),
            ..Fake::with(&["DejaVu Sans", "DejaVu Sans Mono"])
        };
        let (defs, choice) = resolve(&mut f, &LINUX);
        assert_eq!(
            choice.proportional,
            FontSource::System("DejaVu Sans".into())
        );
        assert_eq!(
            choice.monospace,
            FontSource::System("DejaVu Sans Mono".into())
        );
        assert!(
            !defs.font_data.contains_key("sans-serif") && !defs.font_data.contains_key("Noto Sans")
        );
        // Windows: a broken Segoe UI file falls to the generic family (Arial), the broken Consolas to Segoe UI's
        // stand-in.
        let mut w = Fake {
            broken: vec!["Segoe UI", "Consolas"],
            sans: Some(("Arial", UBUNTU)),
            ..Fake::with(&["Segoe UI Symbol"])
        };
        let (wdefs, wchoice) = resolve(&mut w, &WINDOWS);
        assert_eq!(wchoice.proportional, FontSource::System("Arial".into()));
        assert_eq!(
            wdefs.families[&egui::FontFamily::Monospace][0],
            "sans-serif"
        );
        // Whatever was chosen, epaint accepts it (it panics on a face it cannot parse, family.rs:44-50).
        for d in [defs, wdefs] {
            let mut fonts = egui::epaint::text::Fonts::new(Default::default(), d);
            assert!(
                fonts.has_glyph(&egui::FontId::proportional(14.0), 'a')
                    && fonts.has_glyph(&egui::FontId::monospace(14.0), 'a')
            );
        }
    }

    #[test]
    fn every_choice_has_the_right_message() {
        let sys = || FontSource::System("X".into());
        let c = |p, m, f| FontChoice {
            proportional: p,
            monospace: m,
            lookup_failed: f,
        };
        assert_eq!(c(sys(), sys(), false).message(), None);
        assert_eq!(
            c(FontSource::Embedded, FontSource::Embedded, false).message(),
            Some(FLOOR_NO_FONT)
        );
        assert_eq!(
            c(FontSource::Embedded, sys(), false).message(),
            Some(FLOOR_TEXT_ONLY)
        );
        assert_eq!(
            c(FontSource::Embedded, FontSource::Embedded, true).message(),
            Some(FLOOR_LOOKUP_FAILED)
        );
        // Consolas only: the interface font fell back, fixed-width text did not.
        let (_, only_mono) = resolve(&mut Fake::with(&["Consolas"]), &WINDOWS);
        assert_eq!(only_mono.message(), Some(FLOOR_TEXT_ONLY));
        // Rule 3: a proportional system font and no monospace of any kind never gives an embedded monospace.
        for (os, family) in [
            (&WINDOWS, "Segoe UI"),
            (&LINUX, "Noto Sans"),
            (&MACOS, "Helvetica Neue"),
        ] {
            let (_, choice) = resolve(&mut Fake::with(&[family]), os);
            assert_eq!(
                choice.monospace,
                FontSource::System(family.into()),
                "{family}"
            );
            assert_eq!(choice.message(), None);
        }
    }

    // `test-hooks` is on for this target (the self dev-dependency in Cargo.toml), but the cfg keeps the test honest.
    #[cfg(feature = "test-hooks")]
    #[test]
    fn only_sdc_font_embedded_forces_the_floor() {
        use std::ffi::OsStr;
        assert!(floor_forced(Some(OsStr::new("embedded"))));
        for v in ["", "Embedded", "system", "embedded "] {
            assert!(!floor_forced(Some(OsStr::new(v))), "{v:?}");
        }
        assert!(!floor_forced(None));
    }

    /// Every non-ASCII character in the GUI source (this file's tests, its `UI_SYMBOLS` literal and CJK aside) is in
    /// `UI_SYMBOLS`, so the floor-gap and owner pins cover new UI text, and every `UI_SYMBOLS` character is still in
    /// the source. On a failure add or remove the reported characters and rerun the pins.
    #[test]
    fn ui_symbols_cover_the_source() {
        fn rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap().map(Result::unwrap) {
                let p = e.path();
                if p.is_dir() {
                    rs_files(&p, out)
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p)
                }
            }
        }
        let cjk = |c: char| matches!(c as u32, 0x3000..=0x9FFF | 0xAC00..=0xD7AF);
        let mut files = Vec::new();
        rs_files(
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src")),
            &mut files,
        );
        let mut used = std::collections::BTreeSet::new();
        for f in &files {
            let text = std::fs::read_to_string(f).unwrap();
            // This file: only the code before `mod tests` (its user-facing `FLOOR_*` messages), without the
            // `UI_SYMBOLS` literal, which would satisfy the stale check by itself.
            let text = if f.ends_with("fonts.rs") {
                // Cut at the line pair that opens the test module, so a comment naming it cannot end the scan early.
                let lines: Vec<&str> = text.lines().collect();
                let end = lines
                    .windows(2)
                    .position(|w| w[0].starts_with("#[cfg(test)]") && w[1].starts_with("mod tests"))
                    .expect("fonts.rs has a test module");
                lines[..end]
                    .iter()
                    .filter(|l| !l.contains("const UI_SYMBOLS"))
                    .copied()
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                text
            };
            used.extend(text.chars().filter(|c| !c.is_ascii() && !cjk(*c)));
        }
        let missing: String = used.iter().filter(|c| !UI_SYMBOLS.contains(**c)).collect();
        assert!(missing.is_empty(), "add to UI_SYMBOLS: {missing}");
        let stale: String = UI_SYMBOLS.chars().filter(|c| !used.contains(c)).collect();
        assert!(stale.is_empty(), "remove from UI_SYMBOLS: {stale}");
    }

    #[test]
    fn the_windows_table_keeps_todays_chain() {
        let mut f = Fake::with(&["Segoe UI", "Segoe UI Symbol", "Segoe UI Emoji", "Consolas"]);
        let (defs, choice) = resolve(&mut f, &WINDOWS);
        assert_eq!(
            defs.families[&egui::FontFamily::Proportional],
            ["segoeui", "seguisym", "seguiemj"]
        );
        assert_eq!(
            defs.families[&egui::FontFamily::Monospace],
            ["consola", "seguisym"]
        );
        assert_eq!(
            choice,
            FontChoice {
                proportional: FontSource::System("Segoe UI".into()),
                monospace: FontSource::System("Consolas".into()),
                lookup_failed: false
            }
        );
        assert_eq!(choice.message(), None);
        assert!(
            !f.asked.iter().any(|n| n.starts_with('<')),
            "Windows asks the generic family only after Segoe UI: {:?}",
            f.asked
        );
        assert_eq!(
            f.asked.iter().filter(|n| *n == "Segoe UI Symbol").count(),
            1,
            "rule 6: {:?}",
            f.asked
        );
    }

    #[test]
    fn the_linux_lists_are_pinned() {
        let names = |c: &[Candidate]| c.iter().map(|c| c.family).collect::<Vec<_>>();
        assert_eq!(
            [
                LINUX.generic_first,
                WINDOWS.generic_first,
                MACOS.generic_first
            ],
            [true, false, false]
        );
        assert_eq!(
            names(LINUX.proportional),
            [
                "Noto Sans",
                "DejaVu Sans",
                "Liberation Sans",
                "Cantarell",
                "Ubuntu Sans",
                "Ubuntu",
                "Adwaita Sans",
                "Inter",
                "FreeSans"
            ]
        );
        assert_eq!(
            names(LINUX.monospace),
            [
                "DejaVu Sans Mono",
                "Liberation Mono",
                "Noto Sans Mono",
                "Ubuntu Mono"
            ]
        );
        assert_eq!(names(MACOS.proportional), ["SF Pro Text", "Helvetica Neue"]);
        assert_eq!(names(MACOS.monospace), ["Menlo"]);
    }

    #[test]
    fn linux_asks_the_desktops_generic_family_first() {
        let mut f = Fake {
            sans: Some(("Desktop Sans", UBUNTU)),
            mono: Some(("Desktop Mono", HACK)),
            ..Fake::with(&["Noto Sans", "DejaVu Sans Mono"])
        };
        let (defs, choice) = resolve(&mut f, &LINUX);
        assert_eq!(
            defs.families[&egui::FontFamily::Proportional],
            ["sans-serif"]
        );
        assert_eq!(defs.families[&egui::FontFamily::Monospace], ["monospace"]);
        assert_eq!(
            choice,
            FontChoice {
                proportional: FontSource::System("Desktop Sans".into()),
                monospace: FontSource::System("Desktop Mono".into()),
                lookup_failed: false
            }
        );
        assert!(
            !f.asked
                .iter()
                .any(|n| n == "Noto Sans" || n == "DejaVu Sans Mono"),
            "named list not needed: {:?}",
            f.asked
        );
    }

    #[test]
    fn an_unfit_generic_answer_falls_to_the_named_list() {
        for bad_sans in [HACK, EGUI_ICONS] {
            let mut f = Fake {
                sans: Some(("Odd", bad_sans)),
                mono: Some(("Odd Mono", UBUNTU)),
                ..Fake::with(&["Liberation Sans", "DejaVu Sans", "Liberation Mono"])
            };
            let (defs, choice) = resolve(&mut f, &LINUX);
            assert_eq!(
                defs.families[&egui::FontFamily::Proportional][0],
                "DejaVu Sans"
            );
            assert_eq!(
                defs.families[&egui::FontFamily::Monospace][0],
                "Liberation Mono"
            );
            assert_eq!(&f.asked[..3], ["<sans-serif>", "Noto Sans", "DejaVu Sans"]);
            assert!(
                !f.asked.iter().any(|n| n == "Liberation Sans"),
                "stops at the first hit: {:?}",
                f.asked
            );
            assert_eq!(choice.message(), None);
        }
    }

    #[test]
    fn windows_without_segoe_ui_takes_the_generic_family() {
        let mut f = Fake {
            sans: Some(("Arial", UBUNTU)),
            ..Fake::with(&["Consolas"])
        };
        let (defs, choice) = resolve(&mut f, &WINDOWS);
        assert_eq!(
            defs.families[&egui::FontFamily::Proportional][0],
            "sans-serif"
        );
        assert_eq!(choice.proportional, FontSource::System("Arial".into()));
        assert_eq!(f.asked[0], "Segoe UI");
    }

    #[test]
    fn no_font_at_all_uses_the_floor_and_says_so() {
        let (defs, choice) = resolve(&mut Fake::with(&[]), &LINUX);
        assert_eq!(
            defs.families[&egui::FontFamily::Proportional],
            ["Ubuntu-Light", "Hack", "egui-icons"]
        );
        assert_eq!(
            defs.families[&egui::FontFamily::Monospace],
            ["Hack", "Ubuntu-Light", "egui-icons"]
        );
        assert_eq!(
            choice,
            FontChoice {
                proportional: FontSource::Embedded,
                monospace: FontSource::Embedded,
                lookup_failed: false
            }
        );
        assert_eq!(choice.message(), Some(FLOOR_NO_FONT));
        // Rule 4: egui's floor fonts under egui's names, egui-icons at egui's own scale.
        assert_eq!(defs.font_data["Ubuntu-Light"].bytes(), UBUNTU);
        assert_eq!(defs.font_data["Hack"].bytes(), HACK);
        assert_eq!(defs.font_data["egui-icons"].bytes(), EGUI_ICONS);
        assert_eq!(defs.font_data["egui-icons"].tweak.scale, 0.90);
        assert_eq!(embedded().1, choice);
    }

    /// Review Focus 4 and ruling 3: Windows without Consolas keeps today's order (Segoe UI), even though the system
    /// has a generic monospace answer; the generic answer is not asked before Segoe UI.
    #[test]
    fn monospace_without_a_mono_family_uses_the_proportional_one() {
        let mut f = Fake {
            mono: Some(("Courier New", HACK)),
            ..Fake::with(&["Segoe UI", "Segoe UI Symbol"])
        };
        let (defs, choice) = resolve(&mut f, &WINDOWS);
        assert_eq!(
            defs.families[&egui::FontFamily::Monospace],
            ["segoeui", "seguisym"]
        );
        assert_eq!(choice.monospace, FontSource::System("Segoe UI".into()));
        assert_eq!(choice.message(), None);
        assert!(!f.asked.iter().any(|n| n == "<monospace>"), "{:?}", f.asked);
        // With the floor as the interface font, the generic monospace answer is used before the floor's Hack.
        let mut g = Fake {
            mono: Some(("Courier New", HACK)),
            ..Fake::with(&[])
        };
        let (gdefs, gchoice) = resolve(&mut g, &WINDOWS);
        assert_eq!(gdefs.families[&egui::FontFamily::Monospace][0], "monospace");
        assert_eq!(gchoice.monospace, FontSource::System("Courier New".into()));
    }

    /// Review Focus 3: the floor parses, draws a box (no panic) for what it lacks, and lacks exactly these.
    #[test]
    fn the_floor_parses_and_lacks_only_these_ui_symbols() {
        let mut fonts = egui::epaint::text::Fonts::new(Default::default(), embedded().0);
        let id = egui::FontId::proportional(14.0);
        let mut got: Vec<char> = UI_SYMBOLS
            .chars()
            .filter(|c| !fonts.has_glyph(&id, *c))
            .collect();
        let mut want: Vec<char> = "ẞⓘ⚠✎✓✕✖🔒🔗⏵".chars().collect();
        want.sort();
        got.sort();
        assert_eq!(got, want, "floor gaps changed");
        let g = fonts.with_pixels_per_point(1.0).layout_no_wrap(
            "⚠ ✓ 🔒".into(),
            id,
            egui::Color32::WHITE,
        );
        assert!(g.size().x > 0.0);
    }

    #[test]
    fn every_table_resolves_to_definitions_epaint_accepts() {
        for os in [&WINDOWS, &LINUX, &MACOS] {
            let all: Vec<&'static str> = [
                os.proportional,
                os.proportional_extra,
                os.monospace,
                os.monospace_extra,
            ]
            .concat()
            .iter()
            .map(|c| c.family)
            .collect();
            let (defs, _) = resolve(&mut Fake::with(&all), os);
            let mut fonts = egui::epaint::text::Fonts::new(Default::default(), defs);
            assert!(fonts.has_glyph(&egui::FontId::monospace(14.0), 'a'));
        }
    }

    /// The context's installed definitions are the system lookup's: Segoe UI first, and a monospace family.
    #[cfg(windows)]
    #[test]
    fn system_fonts_install_and_fill_the_proportional_family() {
        let ctx = egui::Context::default();
        install_system_fonts(&ctx).expect("Windows system fonts");
        // A dropped `FullOutput` with unapplied texture deltas panics in a debug build (mistakes/gui.md).
        ctx.run_ui(Default::default(), |_| {})
            .textures_delta
            .clear();
        let installed = ctx.fonts(|f| f.definitions().families.clone());
        let want = system_definitions().0;
        assert_eq!(
            installed[&egui::FontFamily::Proportional][0],
            want.families[&egui::FontFamily::Proportional][0]
        );
        assert_eq!(installed[&egui::FontFamily::Proportional][0], "segoeui");
        assert!(!installed[&egui::FontFamily::Monospace].is_empty());
    }

    /// Spec 5.4 / Review Focus 5: CJK comes from the system in the same call that asks for it, with no eager CJK font.
    #[cfg(windows)]
    #[test]
    fn cjk_comes_from_the_system_without_eager_loads() {
        let dir = windows_fonts_dir();
        let defs = system_definitions().0;
        for key in ["msyh", "yugothm", "malgun"] {
            assert!(
                !defs.font_data.contains_key(key),
                "{key} is still loaded at start"
            );
        }
        if ["msyh.ttc", "YuGothM.ttc", "malgun.ttf"]
            .iter()
            .any(|f| dir.join(f).exists())
        {
            let mut fonts = egui::epaint::text::Fonts::new(Default::default(), defs)
                .with_font_providers(vec![system_provider()]);
            assert!(fonts.has_glyphs(&egui::FontId::proportional(14.0), "中文 日本語 한국어"));
            let found = fonts.discovered_fonts();
            assert!(
                !found.is_empty() && found.iter().all(|f| f.name.starts_with("system:")),
                "{:?}",
                found.iter().map(|f| &f.name).collect::<Vec<_>>()
            );
            eprintln!(
                "CJK fonts: {:?}",
                found.iter().map(|f| &f.name).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn one_provider_serves_every_context() {
        assert!(Arc::ptr_eq(&system_provider(), &system_provider()));
    }

    /// Review Focus 1: install replaces the providers, so eframe's stub or another provider is never asked.
    /// U+10FFFD is mapped by none of today's configured fonts on this machine (segoeui, seguisym, seguiemj, consola,
    /// msyh, YuGothM, malgun: checked 2026-11-16), so before the fix the counting provider is asked and the test is
    /// RED; a common Han character would not do (msyh has it) and nor would U+E000 (seguisym has it). The
    /// precondition is asserted, not assumed.
    #[test]
    fn install_replaces_other_providers() {
        const NOWHERE: char = '\u{10FFFD}';
        let asked = Arc::new(AtomicUsize::new(0));
        let ctx = egui::Context::default();
        let a = asked.clone();
        ctx.add_font_provider(Arc::new(
            move |_: &egui::FallbackRequest<'_>| -> Option<egui::FontInsert> {
                a.fetch_add(1, Ordering::Relaxed);
                None
            },
        ));
        install_system_fonts(&ctx).expect("system fonts");
        install_system_fonts(&ctx).expect("system fonts");
        // A dropped `FullOutput` with unapplied texture deltas panics in a debug build (mistakes/gui.md).
        ctx.run_ui(Default::default(), |_| {})
            .textures_delta
            .clear();
        let configured = ctx.fonts(|f| f.definitions().clone());
        let mut plain = egui::epaint::text::Fonts::new(Default::default(), configured);
        assert!(
            !plain.has_glyph(&egui::FontId::proportional(14.0), NOWHERE),
            "precondition: pick a character no configured font maps"
        );
        ctx.fonts_mut(|f| f.has_glyph(&egui::FontId::proportional(14.0), NOWHERE));
        assert_eq!(
            asked.load(Ordering::Relaxed),
            0,
            "a provider added before install was asked"
        );
    }

    /// Review Focus 2.
    #[test]
    fn a_provider_that_panics_answers_none_from_then_on() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let g = Guarded::new(
            move |_: &egui::FallbackRequest<'_>| -> Option<egui::FontInsert> {
                c.fetch_add(1, Ordering::Relaxed);
                panic!("enumeration thread gone")
            },
        );
        let req = egui::FallbackRequest {
            cluster: "日",
            family: &egui::FontFamily::Proportional,
        };
        assert!(g.font_for(&req).is_none());
        assert!(g.font_for(&req).is_none());
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    /// `Guarded` passes on an inner provider's up-front fonts until the provider panics.
    #[test]
    fn a_guarded_provider_keeps_its_up_front_fonts_until_it_panics() {
        struct UpFront;
        impl egui::FontProvider for UpFront {
            fn fonts_for_family(&self, family: &egui::FontFamily) -> Vec<egui::FontInsert> {
                vec![egui::FontInsert::new(
                    "hack",
                    egui::FontData::from_static(HACK),
                    vec![egui::InsertFontFamily {
                        family: family.clone(),
                        priority: egui::FontPriority::Lowest,
                    }],
                )]
            }
            fn font_for(&self, _: &egui::FallbackRequest<'_>) -> Option<egui::FontInsert> {
                panic!("enumeration thread gone")
            }
        }
        let g = Guarded::new(UpFront);
        let family = egui::FontFamily::Proportional;
        assert_eq!(g.fonts_for_family(&family).len(), 1);
        let req = egui::FallbackRequest {
            cluster: "日",
            family: &family,
        };
        assert!(g.font_for(&req).is_none());
        assert!(g.fonts_for_family(&family).is_empty());
    }

    /// Review Important 1: a provider's face that does not parse is dropped, so epaint never gets it (it panics on a
    /// face it cannot parse); dropping one is not a provider failure, so the provider is still asked afterwards.
    #[test]
    fn a_provider_face_that_does_not_parse_is_dropped() {
        fn insert(name: &str, bytes: &'static [u8], family: &egui::FontFamily) -> egui::FontInsert {
            egui::FontInsert::new(
                name,
                egui::FontData::from_static(bytes),
                vec![egui::InsertFontFamily {
                    family: family.clone(),
                    priority: egui::FontPriority::Lowest,
                }],
            )
        }
        /// Answers `JUNK` (and Hack up front), counting its fallback lookups.
        struct Junk(Arc<AtomicUsize>);
        impl egui::FontProvider for Junk {
            fn fonts_for_family(&self, family: &egui::FontFamily) -> Vec<egui::FontInsert> {
                vec![insert("junk", JUNK, family), insert("hack", HACK, family)]
            }
            fn font_for(&self, request: &egui::FallbackRequest<'_>) -> Option<egui::FontInsert> {
                self.0.fetch_add(1, Ordering::Relaxed);
                Some(insert("junk-fallback", JUNK, request.family))
            }
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let g = Guarded::new(Junk(calls.clone()));
        let family = egui::FontFamily::Proportional;
        let req = egui::FallbackRequest {
            cluster: "日",
            family: &family,
        };
        assert!(g.font_for(&req).is_none());
        assert!(g.font_for(&req).is_none());
        assert_eq!(
            calls.load(Ordering::Relaxed),
            2,
            "a dropped face set the failed latch"
        );
        let up_front: Vec<String> = g
            .fonts_for_family(&family)
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert_eq!(up_front, ["hack"]);
        // epaint with that provider: a character the floor lacks lays out (as a box) without a panic.
        let mut fonts = egui::epaint::text::Fonts::new(Default::default(), embedded().0)
            .with_font_providers(vec![Arc::new(Guarded::new(Junk(calls.clone())))]);
        let galley = fonts.with_pixels_per_point(1.0).layout_no_wrap(
            "日".into(),
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );
        assert!(galley.size().x > 0.0);
    }

    /// Start-time budget (Note JM1): fonts installed and one pass laid out, in a fresh process. Run in release, five times.
    #[test]
    #[ignore = "timing; run with --release -- --ignored --nocapture"]
    fn prints_the_font_start_time() {
        let t = std::time::Instant::now();
        let ctx = egui::Context::default();
        install_system_fonts(&ctx).expect("system fonts");
        ctx.run_ui(Default::default(), |ui| {
            ui.label("Cutout + Blend · Coat normal ↺ ▾ ⚠ 🔒 — ×");
        })
        .textures_delta
        .clear();
        eprintln!("font start: {} ms", t.elapsed().as_millis());
    }
}
