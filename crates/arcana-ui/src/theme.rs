//! Theme presets and accent colours (C# `ThemeService`).
//!
//! The nine C# presets are kept with their exact colours. A preset gives the base (light,
//! dark or follow-the-system) and the palette; an optional accent override replaces only the
//! accent, so "base + accent" can be combined freely.

use egui::{Color32, Context, Theme, ThemePreference, Visuals};

/// Base brightness of a preset (C# `ElementTheme`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseTheme {
    System,
    Light,
    Dark,
}

/// One preset (C# `ThemeDefinition`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeDef {
    pub id: &'static str,
    pub name_key: &'static str,
    pub base: BaseTheme,
    pub accent: [u8; 3],
    pub background: [u8; 3],
    pub pane: [u8; 3],
    pub text_primary: [u8; 3],
    pub text_secondary: [u8; 3],
}

const fn def(
    id: &'static str,
    name_key: &'static str,
    base: BaseTheme,
    colors: [[u8; 3]; 5],
) -> ThemeDef {
    ThemeDef {
        id,
        name_key,
        base,
        accent: colors[0],
        background: colors[1],
        pane: colors[2],
        text_primary: colors[3],
        text_secondary: colors[4],
    }
}

/// C# `ThemeService._themes`, in order. The first entry is the default.
pub const THEMES: [ThemeDef; 9] = [
    def(
        "System",
        "settings.theme.system",
        BaseTheme::System,
        [
            [0, 120, 212],
            [249, 249, 249],
            [243, 243, 243],
            [0, 0, 0],
            [96, 96, 96],
        ],
    ),
    def(
        "Light",
        "settings.theme.light",
        BaseTheme::Light,
        [
            [0, 120, 212],
            [255, 255, 255],
            [243, 243, 243],
            [0, 0, 0],
            [96, 96, 96],
        ],
    ),
    def(
        "Dark",
        "settings.theme.dark",
        BaseTheme::Dark,
        [
            [96, 205, 255],
            [32, 32, 32],
            [40, 40, 40],
            [255, 255, 255],
            [180, 180, 180],
        ],
    ),
    def(
        "OceanBlue",
        "settings.theme.oceanblue",
        BaseTheme::Light,
        [
            [0, 120, 212],
            [240, 248, 255],
            [220, 238, 251],
            [26, 58, 92],
            [74, 106, 140],
        ],
    ),
    def(
        "ForestGreen",
        "settings.theme.forestgreen",
        BaseTheme::Light,
        [
            [46, 125, 50],
            [241, 248, 233],
            [220, 237, 200],
            [27, 61, 31],
            [62, 107, 67],
        ],
    ),
    def(
        "PurpleNight",
        "settings.theme.purplenight",
        BaseTheme::Dark,
        [
            [156, 39, 176],
            [26, 26, 46],
            [15, 15, 26],
            [232, 232, 240],
            [168, 168, 192],
        ],
    ),
    def(
        "SunsetOrange",
        "settings.theme.sunsetorange",
        BaseTheme::Light,
        [
            [255, 107, 53],
            [255, 248, 240],
            [255, 224, 178],
            [74, 44, 23],
            [122, 92, 71],
        ],
    ),
    def(
        "RosePink",
        "settings.theme.rosepink",
        BaseTheme::Light,
        [
            [233, 30, 99],
            [255, 240, 245],
            [248, 187, 217],
            [74, 28, 50],
            [122, 76, 98],
        ],
    ),
    def(
        "MidnightBlue",
        "settings.theme.midnightblue",
        BaseTheme::Dark,
        [
            [0, 188, 212],
            [13, 27, 42],
            [10, 22, 40],
            [224, 247, 250],
            [128, 222, 234],
        ],
    ),
];

/// Accent overrides offered on the settings page (Windows accent palette).
pub const ACCENTS: [[u8; 3]; 8] = [
    [0, 120, 212],
    [16, 124, 16],
    [136, 23, 152],
    [232, 17, 35],
    [255, 140, 0],
    [0, 153, 188],
    [194, 57, 179],
    [118, 118, 118],
];

/// Preset by id; unknown ids fall back to the first (C# `?? _themes[0]`).
#[must_use]
pub fn find(id: &str) -> &'static ThemeDef {
    THEMES.iter().find(|t| t.id == id).unwrap_or(&THEMES[0])
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Resolved palette used by views (cards, badges, headings).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub dark: bool,
    pub accent: Color32,
    pub background: Color32,
    pub pane: Color32,
    pub card: Color32,
    pub text: Color32,
    pub text_weak: Color32,
    pub success: Color32,
    pub info: Color32,
}

/// Effective palette for `theme` with optional accent override; `system_dark` decides the
/// base of the `System` preset.
#[must_use]
pub fn palette(theme: &ThemeDef, accent: Option<usize>, system_dark: bool) -> Palette {
    let dark = match theme.base {
        BaseTheme::System => system_dark,
        BaseTheme::Light => false,
        BaseTheme::Dark => true,
    };
    // The System preset follows the OS: in dark mode use the Dark preset's surfaces.
    let surfaces = if theme.base == BaseTheme::System && dark {
        &THEMES[2]
    } else {
        theme
    };
    let accent = accent
        .and_then(|i| ACCENTS.get(i))
        .map_or(rgb(theme.accent), |c| rgb(*c));
    let bg = rgb(surfaces.background);
    let card = if dark {
        bg.blend(Color32::from_white_alpha(12))
    } else {
        bg.blend(Color32::from_white_alpha(140))
    };
    Palette {
        dark,
        accent,
        background: bg,
        pane: rgb(surfaces.pane),
        card,
        text: rgb(surfaces.text_primary),
        text_weak: rgb(surfaces.text_secondary),
        success: Color32::from_rgb(16, 110, 16),
        info: Color32::from_rgb(64, 180, 240),
    }
}

/// egui visuals for a palette.
#[must_use]
pub fn visuals(p: &Palette) -> Visuals {
    let mut v = if p.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    v.panel_fill = p.background;
    v.window_fill = p.pane;
    v.faint_bg_color = p.pane;
    v.override_text_color = Some(p.text);
    v.weak_text_color = Some(p.text_weak);
    v.hyperlink_color = p.accent;
    v.selection.bg_fill = p.accent.gamma_multiply(0.6);
    v.selection.stroke.color = p.accent;
    v.widgets.hovered.bg_stroke.color = p.accent;
    v.widgets.active.bg_fill = p.accent.gamma_multiply(0.8);
    v
}

/// Apply a preset + accent to the egui context.
pub fn apply(ctx: &Context, theme: &ThemeDef, accent: Option<usize>) {
    let light = palette(theme, accent, false);
    let dark = palette(theme, accent, true);
    match theme.base {
        BaseTheme::System => {
            ctx.set_visuals_of(Theme::Light, visuals(&light));
            ctx.set_visuals_of(Theme::Dark, visuals(&dark));
            ctx.set_theme(ThemePreference::System);
        }
        BaseTheme::Light => {
            ctx.set_visuals_of(Theme::Light, visuals(&light));
            ctx.set_theme(Theme::Light);
        }
        BaseTheme::Dark => {
            ctx.set_visuals_of(Theme::Dark, visuals(&dark));
            ctx.set_theme(Theme::Dark);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_match_csharp_ids_and_fallback() {
        let ids: Vec<_> = THEMES.iter().map(|t| t.id).collect();
        assert_eq!(
            ids,
            [
                "System",
                "Light",
                "Dark",
                "OceanBlue",
                "ForestGreen",
                "PurpleNight",
                "SunsetOrange",
                "RosePink",
                "MidnightBlue"
            ]
        );
        assert_eq!(find("Dark").base, BaseTheme::Dark);
        assert_eq!(find("Nope").id, "System");
        assert_eq!(find("ForestGreen").accent, [46, 125, 50]);
    }

    #[test]
    fn palette_resolves_base_and_accent_override() {
        let forest = find("ForestGreen");
        let p = palette(forest, None, true);
        assert!(!p.dark, "a light preset ignores the system");
        assert_eq!(p.accent, Color32::from_rgb(46, 125, 50));
        let p = palette(forest, Some(3), false);
        assert_eq!(p.accent, Color32::from_rgb(232, 17, 35));
        assert_eq!(
            palette(forest, Some(99), false).accent,
            Color32::from_rgb(46, 125, 50)
        );

        let sys = find("System");
        assert!(palette(sys, None, true).dark);
        assert_eq!(
            palette(sys, None, true).background,
            Color32::from_rgb(32, 32, 32)
        );
        assert!(!palette(sys, None, false).dark);
        assert!(palette(find("MidnightBlue"), None, false).dark);
    }

    #[test]
    fn visuals_follow_palette_and_apply_sets_preference() {
        let p = palette(find("Dark"), None, false);
        let v = visuals(&p);
        assert!(v.dark_mode);
        assert_eq!(v.panel_fill, Color32::from_rgb(32, 32, 32));
        assert_eq!(v.hyperlink_color, p.accent);

        let ctx = Context::default();
        apply(&ctx, find("Dark"), None);
        assert_eq!(ctx.theme(), Theme::Dark);
        apply(&ctx, find("Light"), Some(1));
        assert_eq!(ctx.theme(), Theme::Light);
        apply(&ctx, find("System"), None);
    }
}
