//! Version helpers shared by the host: lenient manifest versions and the WASM ABI gate.

use arcana_core::{AppError, AppResult, ErrorCode};
use arcana_plugin_api::PluginManifest;

/// WIT world ABI version this host implements (see `arcana-plugin-wasm`).
pub const HOST_ABI_VERSION: u32 = 1;

/// C# `Version.TryParse(..) ? v : new Version(1, 0, 0)` with 2–4 parts mapped onto `SemVer`
/// (`1.2` → `1.2.0`, the 4th `.NET` part is dropped). Full `SemVer` strings parse as-is.
#[must_use]
pub fn parse_lenient(s: &str) -> semver::Version {
    if let Ok(v) = semver::Version::parse(s.trim()) {
        return v;
    }
    let parts: Vec<u64> = s
        .trim()
        .split('.')
        .map_while(|p| p.parse::<u64>().ok())
        .collect();
    if (2..=4).contains(&parts.len()) && parts.len() == s.trim().split('.').count() {
        semver::Version::new(parts[0], parts[1], parts.get(2).copied().unwrap_or(0))
    } else {
        semver::Version::new(1, 0, 0)
    }
}

/// A WASM plugin (`main` ends in `.wasm`) must declare the host's ABI version.
///
/// # Errors
/// `PluginIncompatible` when the declared ABI differs or is missing.
pub fn validate_abi(m: &PluginManifest) -> AppResult<()> {
    let is_wasm = m
        .main
        .as_deref()
        .is_some_and(|f| f.to_ascii_lowercase().ends_with(".wasm"));
    if !is_wasm {
        return Ok(());
    }
    match m.abi_version {
        Some(v) if v == HOST_ABI_VERSION => Ok(()),
        other => Err(AppError::Plugin {
            code: ErrorCode::PluginIncompatible,
            message: format!(
                "Plugin ABI version {} is not supported (host: {HOST_ABI_VERSION})",
                other.map_or_else(|| "missing".to_string(), |v| v.to_string())
            ),
            plugin_id: Some(m.id.clone()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lenient_versions() {
        assert_eq!(parse_lenient("1.2"), semver::Version::new(1, 2, 0));
        assert_eq!(parse_lenient("1.2.3.4"), semver::Version::new(1, 2, 3));
        assert_eq!(
            parse_lenient("2.0.0-beta"),
            semver::Version::parse("2.0.0-beta").unwrap_or_else(|_| semver::Version::new(0, 0, 0))
        );
        assert_eq!(parse_lenient("abc"), semver::Version::new(1, 0, 0));
        assert_eq!(parse_lenient("1"), semver::Version::new(1, 0, 0));
    }

    #[test]
    fn abi_gate_only_for_wasm() {
        let mut m = PluginManifest {
            id: "x".into(),
            name: "X".into(),
            version: "1.0.0".into(),
            main: Some("x.dll".into()),
            ..PluginManifest::default()
        };
        assert!(validate_abi(&m).is_ok());
        m.main = Some("x.wasm".into());
        assert!(validate_abi(&m).is_err());
        m.abi_version = Some(HOST_ABI_VERSION);
        assert!(validate_abi(&m).is_ok());
        m.abi_version = Some(HOST_ABI_VERSION + 1);
        assert!(validate_abi(&m).is_err());
    }
}
