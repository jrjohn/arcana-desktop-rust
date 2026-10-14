//! CJK glyphs for zh-TW / ja-JP.
//!
//! egui's bundled fonts have no CJK coverage. Fonts are not embedded (size, licensing);
//! the first system font found from a per-OS list is added as a fallback after the default
//! fonts. Without one, Chinese/Japanese text renders as boxes (logged once).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};

/// Well-known CJK-capable system fonts (macOS, Windows, Linux).
pub const CANDIDATES: &[&str] = &[
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/Fonts/STHeiti Medium.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "/Library/Fonts/Arial Unicode.ttf",
    "C:\\Windows\\Fonts\\msjh.ttc",
    "C:\\Windows\\Fonts\\msyh.ttc",
    "C:\\Windows\\Fonts\\YuGothM.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
];

/// Font definitions = egui defaults + `bytes` as fallback for both families.
#[must_use]
pub fn with_fallback(bytes: Vec<u8>) -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
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
    fonts
}

/// Install the first readable font from `candidates`; returns its path.
pub fn install_from(ctx: &egui::Context, candidates: &[&str]) -> Option<PathBuf> {
    for c in candidates {
        let path = Path::new(c);
        if let Ok(bytes) = std::fs::read(path) {
            ctx.set_fonts(with_fallback(bytes));
            tracing::info!("CJK font: {}", path.display());
            return Some(path.to_path_buf());
        }
    }
    tracing::warn!("no CJK system font found; zh-TW / ja-JP text will not render");
    None
}

/// [`install_from`] over [`CANDIDATES`].
#[allow(clippy::must_use_candidate)]
pub fn install(ctx: &egui::Context) -> Option<PathBuf> {
    install_from(ctx, CANDIDATES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fonts_are_skipped_and_fallback_is_appended() {
        let ctx = egui::Context::default();
        assert_eq!(install_from(&ctx, &["/nope/a.ttc"]), None);
        let defs = with_fallback(vec![0, 1, 2]);
        assert!(defs.font_data.contains_key("cjk"));
        assert_eq!(
            defs.families[&FontFamily::Proportional]
                .last()
                .map(String::as_str),
            Some("cjk")
        );
        let _ = install(&ctx);
    }
}
