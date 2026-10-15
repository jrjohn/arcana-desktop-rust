//! CJK glyphs for zh-TW / ja-JP.
//!
//! egui's bundled fonts have no CJK coverage. Fonts are not embedded (size, licensing); the
//! first font found from the running OS's priority list is added as a fallback after the
//! default fonts. Without one, Chinese/Japanese text renders as boxes: the shell shows a
//! one-time warning in the status bar ([`FontStatus::Missing`]).
//!
//! Selection is a pure function over an injectable "does this file exist" predicate, so the
//! per-OS priority is unit-tested without the fonts installed.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};

/// Operating-system family whose font list applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
    Linux,
}

impl Os {
    /// The OS this binary was built for (other Unixes use the Linux list).
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Linux
        }
    }
}

/// One candidate: a font family name (for the log / status bar) and where it lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    pub family: &'static str,
    /// Absolute path; on Windows relative to the fonts directory (`%WINDIR%\Fonts`).
    pub file: &'static str,
}

const fn c(family: &'static str, file: &'static str) -> Candidate {
    Candidate { family, file }
}

/// Windows: `Microsoft JhengHei` (zh-TW), `Yu Gothic UI` (ja-JP), `Microsoft YaHei` (zh-CN).
pub const WINDOWS: &[Candidate] = &[
    c("Microsoft JhengHei", "msjh.ttc"),
    c("Microsoft JhengHei", "msjhl.ttc"),
    c("Yu Gothic UI", "YuGothM.ttc"),
    c("Yu Gothic UI", "YuGothR.ttc"),
    c("Microsoft YaHei", "msyh.ttc"),
];

/// macOS: `PingFang TC`, then `Hiragino Sans`.
pub const MACOS: &[Candidate] = &[
    c("PingFang TC", "/System/Library/Fonts/PingFang.ttc"),
    c(
        "PingFang TC",
        "/System/Library/AssetsV2/com_apple_MobileAsset_Font7/PingFang.ttc",
    ),
    c(
        "Hiragino Sans",
        "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
    ),
    c(
        "Hiragino Sans",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
    ),
    c("Hiragino Sans", "/System/Library/Fonts/STHeiti Medium.ttc"),
];

/// Linux: `Noto Sans CJK` (Debian/Ubuntu, Arch, Fedora layouts), then `WenQuanYi`.
pub const LINUX: &[Candidate] = &[
    c(
        "Noto Sans CJK",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    ),
    c(
        "Noto Sans CJK",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    ),
    c(
        "Noto Sans CJK",
        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
    ),
    c(
        "Noto Sans CJK",
        "/usr/share/fonts/google-noto-sans-cjk-fonts/NotoSansCJK-Regular.ttc",
    ),
    c(
        "WenQuanYi Micro Hei",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    ),
    c(
        "WenQuanYi Zen Hei",
        "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
    ),
    c(
        "WenQuanYi Micro Hei",
        "/usr/share/fonts/wenquanyi/wqy-microhei/wqy-microhei.ttc",
    ),
];

/// Priority list for `os`.
#[must_use]
pub const fn candidates(os: Os) -> &'static [Candidate] {
    match os {
        Os::Windows => WINDOWS,
        Os::MacOs => MACOS,
        Os::Linux => LINUX,
    }
}

/// Full path of a candidate; Windows file names resolve under `windows_dir\Fonts`.
#[must_use]
pub fn resolve(os: Os, cand: &Candidate, windows_dir: &Path) -> PathBuf {
    match os {
        Os::Windows => windows_dir.join("Fonts").join(cand.file),
        Os::MacOs | Os::Linux => PathBuf::from(cand.file),
    }
}

/// First candidate of `os` for which `exists` holds, with its resolved path.
pub fn select(
    os: Os,
    windows_dir: &Path,
    exists: impl Fn(&Path) -> bool,
) -> Option<(Candidate, PathBuf)> {
    candidates(os)
        .iter()
        .map(|cand| (*cand, resolve(os, cand, windows_dir)))
        .find(|(_, p)| exists(p))
}

/// Outcome of [`install`], for the shell's status bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontStatus {
    Found {
        family: String,
        path: PathBuf,
    },
    /// No CJK font: zh-TW / ja-JP text will render as boxes.
    Missing,
}

/// Font definitions = egui defaults + `bytes` as fallback for both families.
#[must_use]
pub fn with_fallback(bytes: Vec<u8>) -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    arcana_plugin_flowchart::ui::add_icon_font(&mut fonts);
    add_fallback(&mut fonts, bytes);
    fonts
}

/// Append `bytes` as the `cjk` fallback of both families of `fonts`.
pub fn add_fallback(fonts: &mut FontDefinitions, bytes: Vec<u8>) {
    fonts
        .font_data
        .insert("cjk".to_owned(), Arc::new(FontData::from_owned(bytes)));
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("cjk".to_owned());
    }
}

fn windows_dir() -> PathBuf {
    std::env::var_os("WINDIR")
        .or_else(|| std::env::var_os("SystemRoot"))
        .map_or_else(|| PathBuf::from("C:\\Windows"), PathBuf::from)
}

/// Pick the CJK font for `os` and add it to `fonts`; reading happens through `read`.
pub fn install_into(
    fonts: &mut FontDefinitions,
    os: Os,
    windows_dir: &Path,
    read: impl Fn(&Path) -> Option<Vec<u8>>,
) -> FontStatus {
    for cand in candidates(os) {
        let path = resolve(os, cand, windows_dir);
        if let Some(bytes) = read(&path) {
            add_fallback(fonts, bytes);
            tracing::info!("CJK font: {} ({})", cand.family, path.display());
            return FontStatus::Found {
                family: cand.family.to_owned(),
                path,
            };
        }
    }
    tracing::warn!("no CJK system font found; zh-TW / ja-JP text will not render");
    FontStatus::Missing
}

/// Install egui's defaults plus the current OS's CJK font (and `extra` additions such as an
/// icon font) into `ctx`.
pub fn install_with(ctx: &egui::Context, extra: impl FnOnce(&mut FontDefinitions)) -> FontStatus {
    let mut fonts = FontDefinitions::default();
    // FlowChart toolbar icons (egui-phosphor), whether or not a CJK font exists.
    arcana_plugin_flowchart::ui::add_icon_font(&mut fonts);
    extra(&mut fonts);
    let status = install_into(&mut fonts, Os::current(), &windows_dir(), |p| {
        std::fs::read(p).ok()
    });
    ctx.set_fonts(fonts);
    status
}

/// [`install_with`] without extras.
#[allow(clippy::must_use_candidate)]
pub fn install(ctx: &egui::Context) -> FontStatus {
    install_with(ctx, |_| {})
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn pick(os: Os, present: &[&str]) -> Option<&'static str> {
        let set: HashSet<PathBuf> = present.iter().map(PathBuf::from).collect();
        select(os, Path::new("C:\\Windows"), |p| set.contains(p)).map(|(c, _)| c.family)
    }

    #[test]
    fn windows_prefers_jhenghei_then_yu_gothic_then_yahei() {
        let w = |f: &str| {
            Path::new("C:\\Windows")
                .join("Fonts")
                .join(f)
                .to_string_lossy()
                .into_owned()
        };
        let all = [w("msyh.ttc"), w("YuGothM.ttc"), w("msjh.ttc")];
        let refs: Vec<&str> = all.iter().map(String::as_str).collect();
        assert_eq!(pick(Os::Windows, &refs), Some("Microsoft JhengHei"));
        assert_eq!(pick(Os::Windows, &refs[..2]), Some("Yu Gothic UI"));
        assert_eq!(pick(Os::Windows, &refs[..1]), Some("Microsoft YaHei"));
        assert_eq!(pick(Os::Windows, &[]), None);
    }

    #[test]
    fn macos_prefers_pingfang_then_hiragino() {
        let pf = "/System/Library/Fonts/PingFang.ttc";
        let hs = "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc";
        assert_eq!(pick(Os::MacOs, &[hs, pf]), Some("PingFang TC"));
        assert_eq!(pick(Os::MacOs, &[hs]), Some("Hiragino Sans"));
        // A Linux path never matches on macOS.
        assert_eq!(
            pick(
                Os::MacOs,
                &["/usr/share/fonts/truetype/wqy/wqy-microhei.ttc"]
            ),
            None
        );
    }

    #[test]
    fn linux_prefers_noto_then_wenquanyi() {
        let noto = "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc";
        let wqy = "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc";
        assert_eq!(pick(Os::Linux, &[wqy, noto]), Some("Noto Sans CJK"));
        assert_eq!(pick(Os::Linux, &[wqy]), Some("WenQuanYi Zen Hei"));
        assert_eq!(pick(Os::Linux, &[]), None);
    }

    #[test]
    fn install_into_reports_found_or_missing() {
        let mut defs = FontDefinitions::default();
        let st = install_into(&mut defs, Os::Linux, Path::new("/"), |_| None);
        assert_eq!(st, FontStatus::Missing);
        assert!(!defs.font_data.contains_key("cjk"));
        let st = install_into(&mut defs, Os::Linux, Path::new("/"), |p| {
            p.to_string_lossy()
                .contains("wqy-microhei")
                .then(|| vec![0])
        });
        assert!(
            matches!(st, FontStatus::Found { ref family, .. } if family == "WenQuanYi Micro Hei")
        );
        assert_eq!(
            defs.families[&FontFamily::Proportional]
                .last()
                .map(String::as_str),
            Some("cjk")
        );
    }

    #[test]
    fn with_fallback_appends_cjk() {
        let defs = with_fallback(vec![0, 1, 2]);
        assert!(defs.font_data.contains_key("cjk"));
        let _ = install(&egui::Context::default());
    }
}
