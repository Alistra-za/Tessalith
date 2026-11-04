//! Command-line arguments (spec §8) and their translation into an engine `BatchRequest`.
use clap::{Args, Parser, Subcommand, ValueEnum};
use engine::api::{BatchRequest, OnExisting};
use engine::codes::Code;
use engine::config::Target;
use engine::overrides::parse_set;
use engine::policy::AcceptSuggestions;
use engine::presets::PresetRegistry;
use engine::schemas::SchemaKind;
use std::path::PathBuf;

/// `sdc --version`: the engine version, then each vendored encoder's version, one per line.
fn long_version() -> &'static str {
    static TEXT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    TEXT.get_or_init(|| {
        let v = encoders_sys::versions();
        let mut lines = vec![
            env!("CARGO_PKG_VERSION").to_string(),
            format!("bc7enc_rdo {}", v.bc7enc_rdo),
            format!("ispc_texture_compressor {}", v.ispctc),
            format!("ispc {}", v.ispc),
        ];
        lines.extend(v.bc7e_patch.map(|p| format!("bc7e patch {p}")));
        lines.extend(v.rgbcx_patch.map(|p| format!("rgbcx patch {p}")));
        lines.join("\n")
    })
}

#[derive(Parser)]
#[command(
    name = "sdc",
    version,
    long_version = long_version(),
    about = "sdc — the Tessalith command line: Skyrim SE/AE DDS converter"
)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Convert images or folders to DDS.
    Convert(ConvertArgs),
    /// Analyse inputs; print proposed sub-type, profile and warnings; DDS headers and mip statistics.
    Inspect(InspectArgs),
    /// Run the spec §5.8 checks on existing DDS files.
    Validate(ValidateArgs),
    /// List and show presets, and print schemas. User presets in %APPDATA%\sdc\presets\*.toml are always loaded.
    Presets(PresetsArgs),
    /// Write the 1x1 CS/ENB dynamic cubemap marker DDS (152 bytes).
    CubemapMarker(MarkerArgs),
}

#[derive(Clone, Copy, ValueEnum)]
pub enum TargetArg {
    Cs,
    Vanilla,
}

impl From<TargetArg> for Target {
    fn from(t: TargetArg) -> Target {
        match t {
            TargetArg::Cs => Target::Cs,
            TargetArg::Vanilla => Target::Vanilla,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub enum AcceptArg {
    None,
    High,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum OnExistingArg {
    IfChanged,
    Skip,
    Overwrite,
}

impl From<OnExistingArg> for OnExisting {
    fn from(o: OnExistingArg) -> OnExisting {
        match o {
            OnExistingArg::IfChanged => OnExisting::IfChanged,
            OnExistingArg::Skip => OnExisting::Skip,
            OnExistingArg::Overwrite => OnExisting::Overwrite,
        }
    }
}

#[derive(Args, Clone)]
pub struct BatchArgs {
    /// Image files or folders.
    #[arg(required = true)]
    pub inputs: Vec<PathBuf>,
    /// Target (default: from the configs or the selected preset, finally `cs`).
    #[arg(long, value_enum)]
    pub target: Option<TargetArg>,
    /// Preset id (built-in, or from %APPDATA%\sdc\presets\*.toml), or a path to a preset .toml file.
    #[arg(long)]
    pub preset: Option<String>,
    /// Load a preset .toml without selecting it, after the user presets folder, e.g. one declaring suffix aliases
    /// (repeatable). An id that is already loaded is an error.
    #[arg(long = "preset-file")]
    pub preset_file: Vec<PathBuf>,
    /// Project config (default: sdc.toml in the first input folder).
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Field override: field=value, subtype:<id>.field=value, file:<glob>.field=value. `set:<id>.shader=<shader>`
    /// picks a set's shader (ids as `inspect` prints them after `set`).
    #[arg(long = "set")]
    pub set: Vec<String>,
    #[arg(long, value_enum, default_value = "none")]
    pub accept_suggestions: AcceptArg,
    /// Acknowledge a blocking warning code (repeatable).
    #[arg(long)]
    pub allow: Vec<String>,
    #[arg(long)]
    pub recursive: bool,
    /// Write JSON Lines events to stdout (human output stays on stderr).
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct ConvertArgs {
    #[command(flatten)]
    pub batch: BatchArgs,
    /// Output root (default: ./sdc-out). With `[export] also` in `sdc.toml`, the folder the PBR, Vanilla and CM folders go in.
    #[arg(short = 'o', long)]
    pub output: Option<PathBuf>,
    /// Worker threads (0 = all cores).
    #[arg(long, default_value_t = 0)]
    pub jobs: usize,
    /// Memory budget, e.g. 8G or 512M (default 50% of RAM).
    #[arg(long)]
    pub mem_budget: Option<String>,
    #[arg(long, value_enum, default_value = "if-changed")]
    pub on_existing: OnExistingArg,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct InspectArgs {
    #[command(flatten)]
    pub batch: BatchArgs,
}

#[derive(Args)]
pub struct ValidateArgs {
    #[arg(required = true)]
    pub dds: Vec<PathBuf>,
    /// Target whose rules apply (default cs).
    #[arg(long, value_enum)]
    pub target: Option<TargetArg>,
    /// Sub-type the files are meant to be (default: inferred from the file name).
    #[arg(long)]
    pub subtype: Option<String>,
    /// Alpha policy the files are meant to follow (default: inferred from the stored alpha; `remove` if
    /// every mip-0 alpha is at least 254).
    #[arg(long, value_parser = ["remove", "cutout", "blend", "cutout_blend", "data"])]
    pub alpha_policy: Option<String>,
    /// Hold BC7 files under `remove` to sdc's exact-opaque guarantee (decoded alpha 255, not 254). Off by
    /// default: only sdc's own encoder promises it, so third-party files are not judged by it.
    #[arg(long)]
    pub exact_opaque: bool,
    /// Write one JSON object per file to stdout (the human report goes to stderr).
    #[arg(long)]
    pub json: bool,
}

/// The schema to print. Mirrors `engine::schemas::SchemaKind`, which stays free of clap.
#[derive(Clone, Copy, ValueEnum)]
pub enum SchemaArg {
    All,
    Profile,
    Preset,
    Config,
    Event,
    Manifest,
    DdsInfo,
    DdsValidation,
}

impl From<SchemaArg> for SchemaKind {
    fn from(s: SchemaArg) -> SchemaKind {
        match s {
            SchemaArg::All => SchemaKind::All,
            SchemaArg::Profile => SchemaKind::Profile,
            SchemaArg::Preset => SchemaKind::Preset,
            SchemaArg::Config => SchemaKind::Config,
            SchemaArg::Event => SchemaKind::Event,
            SchemaArg::Manifest => SchemaKind::Manifest,
            SchemaArg::DdsInfo => SchemaKind::DdsInfo,
            SchemaArg::DdsValidation => SchemaKind::DdsValidation,
        }
    }
}

#[derive(Args)]
pub struct PresetsArgs {
    /// Load a preset .toml after the user presets folder (repeatable). An id that is already loaded is an error.
    #[arg(long = "preset-file", global = true)]
    pub preset_file: Vec<PathBuf>,
    #[command(subcommand)]
    pub cmd: PresetsCmd,
}

#[derive(Subcommand)]
pub enum PresetsCmd {
    /// List selectable presets, built-in and user (with their source file).
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show a preset's resolved profile(s) with provenance.
    Show {
        id: String,
        #[arg(long)]
        subtype: Option<String>,
        #[arg(long, value_enum)]
        target: Option<TargetArg>,
    },
    /// Print a JSON Schema (profile, preset, config, event, manifest, dds-info, dds-validation) or write all to a folder.
    Schema {
        #[arg(value_enum)]
        which: SchemaArg,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

/// `--mem-budget`: plain bytes or a `G`/`M` suffix (binary multiples).
pub fn parse_bytes(s: &str) -> Result<u64, String> {
    let s = s.trim();
    let (num, mul) = match s.chars().last() {
        Some('G' | 'g') => (&s[..s.len() - 1], 1u64 << 30),
        Some('M' | 'm') => (&s[..s.len() - 1], 1u64 << 20),
        Some(c) if c.is_ascii_digit() => (s, 1),
        _ => return Err(format!("--mem-budget '{s}': use bytes or a G/M suffix")),
    };
    match num
        .trim()
        .parse::<u64>()
        .ok()
        .and_then(|n| n.checked_mul(mul))
    {
        // The engine reads 0 as "default", so an explicit 0 would silently mean 50% of RAM.
        Some(0) => Err(format!(
            "--mem-budget '{s}': must be greater than 0 (omit the flag for the default, 50% of RAM)"
        )),
        Some(n) => Ok(n),
        None => Err(format!("--mem-budget '{s}': not a valid size")),
    }
}

/// `%APPDATA%\sdc\presets`, the user presets folder (M6 Track D spec §3); `None` when APPDATA is unset.
pub fn user_presets_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("sdc").join("presets"))
}

/// Translate the shared batch flags into a `BatchRequest`. Errors are usage or configuration errors (exit 2).
pub fn build_request(b: &BatchArgs, output: PathBuf) -> Result<BatchRequest, String> {
    // `BatchRequest::new` takes a target; `--target` is optional, so reset it to "not given".
    let mut r = BatchRequest::new(b.inputs.clone(), output, Target::Cs);
    r.target = b.target.map(Target::from);
    match &b.preset {
        Some(p) if p.to_ascii_lowercase().ends_with(".toml") => {
            // `--preset <file.toml>` loads the file and selects the id it declares.
            let path = PathBuf::from(p);
            let id = PresetRegistry::builtin()
                .and_then(|mut reg| reg.add_file(&path))
                .map_err(|e| format!("--preset {p}: {e}"))?;
            r.preset_files.push(path);
            r.preset = Some(id);
        }
        Some(p) => r.preset = Some(p.clone()),
        None => {}
    }
    // `--preset-file` loads without selecting (spec §3.7, §8).
    r.preset_files.extend(b.preset_file.iter().cloned());
    r.project_config = b.config.clone();
    r.user_config =
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("sdc").join("config.toml"));
    r.user_presets = user_presets_dir();
    // Ruling FR-a: every bad `--set` and `--allow` in one error, so a pasted 1.x command is fixed in one pass.
    let mut bad = vec![];
    for s in &b.set {
        match parse_set(s) {
            Ok(o) => r.overrides.push(o),
            Err(e) => bad.push(e),
        }
    }
    for a in &b.allow {
        match Code::parse_allow(a) {
            Ok(c) => {
                r.allow.insert(c);
            }
            Err(e) => bad.push(e),
        }
    }
    if !bad.is_empty() {
        return Err(bad.join("\n"));
    }
    r.accept = match b.accept_suggestions {
        AcceptArg::None => AcceptSuggestions::None,
        AcceptArg::High => AcceptSuggestions::High,
    };
    r.recursive = b.recursive;
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bytes_units() {
        assert_eq!(parse_bytes("512"), Ok(512));
        assert_eq!(parse_bytes("512M"), Ok(512 << 20));
        assert_eq!(parse_bytes("8G"), Ok(8 << 30));
        assert_eq!(parse_bytes(" 2g "), Ok(2 << 30));
        for bad in [
            "",
            "G",
            "8Q",
            "-1",
            "1.5G",
            "99999999999999999999G",
            "0",
            "0G",
            "00M",
        ] {
            assert!(parse_bytes(bad).is_err(), "{bad}");
        }
        assert!(parse_bytes("0").unwrap_err().contains("greater than 0"));
    }
}

#[derive(Args)]
pub struct MarkerArgs {
    /// F0 colour as R,G,B code values 0..255, written as is. CS reads F0 as (code/255)^1.6 (GameGammaToLinear).
    #[arg(long, value_parser = parse_rgb, default_value = "0,0,0")]
    pub color: [u8; 3],
    /// Roughness 0..1 (stored in alpha). Omitted: alpha 255, CS's defaults (F0 1, roughness 1/7).
    #[arg(long)]
    pub roughness: Option<f64>,
    /// The DDS file to write.
    #[arg(short = 'o', long)]
    pub out: PathBuf,
}

fn parse_rgb(s: &str) -> Result<[u8; 3], String> {
    let v: Vec<u8> = s
        .split(',')
        .map(|x| {
            x.trim()
                .parse::<u8>()
                .map_err(|_| format!("{x:?} is not 0..255"))
        })
        .collect::<Result<_, _>>()?;
    <[u8; 3]>::try_from(v).map_err(|_| "expected R,G,B".to_string())
}
