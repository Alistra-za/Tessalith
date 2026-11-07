//! Textual #include expansion so D3DCompile sees one buffer; CS includes are root-relative ("Common/x.hlsli").
use std::path::PathBuf;

/// Where shader text is read from: the tree embedded in the exe. Tests can also read a folder (stand-ins, the
/// owner's CS for comparisons).
#[derive(Clone, Debug, Default)]
pub enum ShaderSource {
    #[default]
    Bundled,
    #[cfg(feature = "test-hooks")]
    Folder(PathBuf),
}

impl ShaderSource {
    fn read(&self, rel: &str) -> Option<Vec<u8>> {
        match self {
            ShaderSource::Bundled => crate::bundled::read(rel).map(<[u8]>::to_vec),
            #[cfg(feature = "test-hooks")]
            ShaderSource::Folder(root) => std::fs::read(root.join(rel)).ok(),
        }
    }

    fn has(&self, rel: &str) -> bool {
        match self {
            ShaderSource::Bundled => crate::bundled::read(rel).is_some(),
            #[cfg(feature = "test-hooks")]
            ShaderSource::Folder(root) => root.join(rel).is_file(),
        }
    }

    /// What error messages and `Expanded::files` call `rel`: the bundle's `rel` itself, or a test folder's path.
    pub fn path_of(&self, rel: &str) -> PathBuf {
        match self {
            ShaderSource::Bundled => PathBuf::from(rel),
            #[cfg(feature = "test-hooks")]
            ShaderSource::Folder(root) => root.join(rel),
        }
    }
}

pub struct Expanded {
    pub text: String,
    pub files: Vec<PathBuf>,
}

fn target(line: &str) -> Option<&str> {
    let rest = line
        .trim_start()
        .strip_prefix('#')?
        .trim_start()
        .strip_prefix("include")?
        .trim();
    let rest = rest.strip_prefix('"').or_else(|| rest.strip_prefix('<'))?;
    rest.split(['"', '>']).next()
}

fn go(source: &ShaderSource, rel: &str, depth: u32, out: &mut Expanded) -> std::io::Result<()> {
    if depth > 64 {
        return Err(std::io::Error::other(format!(
            "include depth > 64 at {rel}"
        )));
    }
    let bytes = source.read(rel).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, format!("{rel} not found"))
    })?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    out.files.push(source.path_of(rel));
    let dir = rel.rsplit_once('/').map(|(d, _)| d);
    for (n, line) in text.lines().enumerate() {
        if let Some(t) = target(line) {
            let t = t.replace('\\', "/");
            // The includer's folder first, then the root.
            let beside = dir.map(|d| format!("{d}/{t}"));
            let found = beside.into_iter().chain([t]).find(|p| source.has(p));
            if let Some(found) = found {
                out.text.push_str(&format!("#line 1 \"{found}\"\n"));
                go(source, &found, depth + 1, out)?;
                out.text.push_str(&format!("#line {} \"{rel}\"\n", n + 2));
                continue;
            }
        }
        out.text.push_str(line);
        out.text.push('\n');
    }
    Ok(())
}

/// `rel` (root-relative, `/` or `\`) with every include inlined. The `#line` names are root-relative with `/`.
pub fn expand(source: &ShaderSource, rel: &str) -> std::io::Result<Expanded> {
    let mut out = Expanded {
        text: String::new(),
        files: vec![],
    };
    go(source, &rel.replace('\\', "/"), 0, &mut out)?;
    Ok(out)
}
