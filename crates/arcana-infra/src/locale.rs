//! System UI language detection (C# `CultureInfo.CurrentUICulture`).

/// The cultures the app ships, fallback first (C# `LocalizationServiceImpl` order).
pub const SUPPORTED_CULTURES: [&str; 3] = ["en-US", "zh-TW", "ja-JP"];

/// The fallback culture.
pub const FALLBACK_CULTURE: &str = "en-US";

/// The system locale as a BCP-47-ish name (`zh_TW.UTF-8` → `zh-TW`), from `LC_ALL`,
/// `LC_MESSAGES` or `LANG`.
#[must_use]
pub fn system_locale() -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find_map(|v| normalize(&v))
}

/// `zh_TW.UTF-8@x` → `zh-TW`; `C` / `POSIX` / empty → `None`.
#[must_use]
pub fn normalize(raw: &str) -> Option<String> {
    let base = raw.split(['.', '@']).next().unwrap_or_default().trim();
    if base.is_empty() || base == "C" || base == "POSIX" {
        return None;
    }
    Some(base.replace('_', "-"))
}

/// Pick a supported culture for `system`: exact (case-insensitive) match first, then same
/// two-letter language, else `en-US` (C# `AppSettings.GetDefaultLanguage`).
#[must_use]
pub fn best_supported(system: Option<&str>) -> &'static str {
    let Some(system) = system else {
        return FALLBACK_CULTURE;
    };
    if let Some(c) = SUPPORTED_CULTURES
        .iter()
        .find(|c| c.eq_ignore_ascii_case(system))
    {
        return c;
    }
    let lang = system.split('-').next().unwrap_or_default();
    SUPPORTED_CULTURES
        .iter()
        .find(|c| {
            c.split('-')
                .next()
                .is_some_and(|l| l.eq_ignore_ascii_case(lang))
        })
        .copied()
        .unwrap_or(FALLBACK_CULTURE)
}
