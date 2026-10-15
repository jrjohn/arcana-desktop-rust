//! Semantic versions and ranges (C# `SemanticVersioning.cs`: `SemanticVersion`,
//! `VersionRange`, `PluginDependency`, `DependencyResolutionResult`, `DependencyConflict`).
//!
//! Behaviour follows C#, not the semver spec: prerelease tags compare ordinally as whole
//! strings, `"1.0"` / `"1.2.3.4"` parse through the .NET `Version.TryParse` fallback, and
//! build metadata is ignored by equality and hashing.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

use arcana_core::{AppError, AppResult, ErrorCode};

fn format_error(what: &str, input: &str) -> AppError {
    AppError::data(ErrorCode::InvalidFormat, format!("Invalid {what}: {input}"))
}

/// C# `SemanticVersion`.
#[derive(Debug, Clone, Eq)]
pub struct SemanticVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub prerelease: Option<String>,
    pub build_metadata: Option<String>,
}

impl SemanticVersion {
    #[must_use]
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            prerelease: None,
            build_metadata: None,
        }
    }

    /// C# constructor with its non-negative checks (`ArgumentException`).
    ///
    /// # Errors
    /// `AppError::Validation` for a negative component.
    pub fn try_new(
        major: i64,
        minor: i64,
        patch: i64,
        prerelease: Option<&str>,
        build_metadata: Option<&str>,
    ) -> AppResult<Self> {
        let part = |v: i64, name: &str| {
            u64::try_from(v).map_err(|_| {
                AppError::validation(
                    format!("{name} version must be non-negative"),
                    vec![name.to_lowercase()],
                )
            })
        };
        Ok(Self {
            major: part(major, "Major")?,
            minor: part(minor, "Minor")?,
            patch: part(patch, "Patch")?,
            prerelease: prerelease.map(str::to_string),
            build_metadata: build_metadata.map(str::to_string),
        })
    }

    /// C# `Parse` (`FormatException` → `Err`).
    ///
    /// # Errors
    /// `ErrorCode::InvalidFormat` when [`Self::try_parse`] fails.
    pub fn parse(version: &str) -> AppResult<Self> {
        Self::try_parse(Some(version)).ok_or_else(|| format_error("semantic version", version))
    }

    /// C# `TryParse`.
    #[must_use]
    pub fn try_parse(version: Option<&str>) -> Option<Self> {
        let v = version?;
        if v.trim().is_empty() {
            return None;
        }
        Self::parse_strict(v).or_else(|| Self::parse_dotnet(v))
    }

    /// `^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z\-\.]+))?(?:\+([0-9A-Za-z\-\.]+))?$`
    fn parse_strict(v: &str) -> Option<Self> {
        let ident = |s: &str| {
            !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
        };
        let (rest, build) = match v.split_once('+') {
            Some((r, b)) => (r, Some(b)),
            None => (v, None),
        };
        let (core, pre) = match rest.split_once('-') {
            Some((c, p)) => (c, Some(p)),
            None => (rest, None),
        };
        if pre.is_some_and(|p| !ident(p)) || build.is_some_and(|b| !ident(b)) {
            return None;
        }
        let nums: Vec<&str> = core.split('.').collect();
        if nums.len() != 3
            || nums
                .iter()
                .any(|n| n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()))
        {
            return None;
        }
        Some(Self {
            major: nums[0].parse().ok()?,
            minor: nums[1].parse().ok()?,
            patch: nums[2].parse().ok()?,
            prerelease: pre.map(str::to_string),
            build_metadata: build.map(str::to_string),
        })
    }

    /// .NET `Version.TryParse`: 2–4 non-negative `int` components.
    fn parse_dotnet(v: &str) -> Option<Self> {
        let parts: Vec<&str> = v.split('.').collect();
        if !(2..=4).contains(&parts.len()) {
            return None;
        }
        let nums: Vec<u64> = parts
            .iter()
            .map(|p| {
                p.trim()
                    .parse::<u64>()
                    .ok()
                    .filter(|n| i32::try_from(*n).is_ok())
            })
            .collect::<Option<_>>()?;
        Some(Self::new(
            nums[0],
            nums[1],
            nums.get(2).copied().unwrap_or(0),
        ))
    }

    /// C# `FromVersion`.
    #[must_use]
    pub fn from_version(v: &semver::Version) -> Self {
        Self::new(v.major, v.minor, v.patch)
    }

    /// C# `ToVersion` (drops prerelease/build like C#).
    #[must_use]
    pub fn to_version(&self) -> semver::Version {
        semver::Version::new(self.major, self.minor, self.patch)
    }
}

impl PartialEq for SemanticVersion {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Hash for SemanticVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.major, self.minor, self.patch, &self.prerelease).hash(state);
    }
}

impl Ord for SemanticVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (&self.prerelease, &other.prerelease) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => a.cmp(b),
            })
    }
}

impl PartialOrd for SemanticVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for SemanticVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(p) = self.prerelease.as_deref().filter(|p| !p.is_empty()) {
            write!(f, "-{p}")?;
        }
        if let Some(b) = self.build_metadata.as_deref().filter(|b| !b.is_empty()) {
            write!(f, "+{b}")?;
        }
        Ok(())
    }
}

/// C# `VersionRange`. `Default` = any version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionRange {
    pub min_version: Option<SemanticVersion>,
    pub max_version: Option<SemanticVersion>,
    pub min_inclusive: bool,
    pub max_inclusive: bool,
}

impl Default for VersionRange {
    fn default() -> Self {
        Self {
            min_version: None,
            max_version: None,
            min_inclusive: true,
            max_inclusive: false,
        }
    }
}

impl VersionRange {
    #[must_use]
    pub fn between(min: SemanticVersion, max: SemanticVersion, max_inclusive: bool) -> Self {
        Self {
            min_version: Some(min),
            max_version: Some(max),
            min_inclusive: true,
            max_inclusive,
        }
    }

    #[must_use]
    pub fn exact(v: SemanticVersion) -> Self {
        Self::between(v.clone(), v, true)
    }

    /// C# `IsSatisfiedBy`.
    #[must_use]
    pub fn is_satisfied_by(&self, version: &SemanticVersion) -> bool {
        if let Some(min) = &self.min_version {
            let c = version.cmp(min);
            if if self.min_inclusive {
                c.is_lt()
            } else {
                c.is_le()
            } {
                return false;
            }
        }
        if let Some(max) = &self.max_version {
            let c = version.cmp(max);
            if if self.max_inclusive {
                c.is_gt()
            } else {
                c.is_ge()
            } {
                return false;
            }
        }
        true
    }

    fn open_min(v: SemanticVersion, inclusive: bool) -> Self {
        Self {
            min_version: Some(v),
            min_inclusive: inclusive,
            ..Self::default()
        }
    }

    fn open_max(v: SemanticVersion, inclusive: bool) -> Self {
        Self {
            max_version: Some(v),
            max_inclusive: inclusive,
            ..Self::default()
        }
    }

    /// C# `VersionRange.Parse`.
    ///
    /// # Errors
    /// `ErrorCode::InvalidFormat` (C# `FormatException`).
    pub fn parse(range: &str) -> AppResult<Self> {
        let r = range.trim();
        if r.is_empty() {
            return Ok(Self::default());
        }
        if let Some(exact) = SemanticVersion::try_parse(Some(r)) {
            return Ok(Self::exact(exact));
        }
        if let Some(rest) = r.strip_prefix('^') {
            let v = SemanticVersion::parse(rest)?;
            let max = SemanticVersion::new(v.major + 1, 0, 0);
            return Ok(Self::between(v, max, false));
        }
        if let Some(rest) = r.strip_prefix('~') {
            let v = SemanticVersion::parse(rest)?;
            let max = SemanticVersion::new(v.major, v.minor + 1, 0);
            return Ok(Self::between(v, max, false));
        }
        if let Some((a, b)) = r.split_once(" - ") {
            return Ok(Self::between(
                SemanticVersion::parse(a.trim())?,
                SemanticVersion::parse(b.trim())?,
                true,
            ));
        }
        if let Some(rest) = r.strip_prefix(">=") {
            return Ok(Self::open_min(SemanticVersion::parse(rest.trim())?, true));
        }
        if let Some(rest) = r.strip_prefix('>') {
            return Ok(Self::open_min(SemanticVersion::parse(rest.trim())?, false));
        }
        if let Some(rest) = r.strip_prefix("<=") {
            return Ok(Self::open_max(SemanticVersion::parse(rest.trim())?, true));
        }
        if let Some(rest) = r.strip_prefix('<') {
            return Ok(Self::open_max(SemanticVersion::parse(rest.trim())?, false));
        }
        if r.ends_with(".*") {
            let replaced = r.replace('*', "0");
            let parts: Vec<&str> = replaced.split('.').collect();
            let num = |s: &str| {
                s.parse::<u64>()
                    .map_err(|_| format_error("version range", r))
            };
            let major = num(parts[0])?;
            if parts.len() == 2 {
                return Ok(Self::between(
                    SemanticVersion::new(major, 0, 0),
                    SemanticVersion::new(major + 1, 0, 0),
                    false,
                ));
            }
            let minor = num(parts[1])?;
            return Ok(Self::between(
                SemanticVersion::new(major, minor, 0),
                SemanticVersion::new(major, minor + 1, 0),
                false,
            ));
        }
        Err(format_error("version range", r))
    }
}

impl fmt::Display for VersionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let (Some(a), Some(b)) = (&self.min_version, &self.max_version)
            && a == b
        {
            return write!(f, "{a}");
        }
        let mut parts = Vec::new();
        if let Some(a) = &self.min_version {
            parts.push(format!(
                "{}{a}",
                if self.min_inclusive { ">=" } else { ">" }
            ));
        }
        if let Some(b) = &self.max_version {
            parts.push(format!(
                "{}{b}",
                if self.max_inclusive { "<=" } else { "<" }
            ));
        }
        if parts.is_empty() {
            f.write_str("*")
        } else {
            f.write_str(&parts.join(" "))
        }
    }
}

/// C# `PluginDependency`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginDependency {
    pub plugin_id: String,
    pub version_range: VersionRange,
    pub optional: bool,
}

impl PluginDependency {
    #[must_use]
    pub fn new(plugin_id: &str, version_range: VersionRange) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            version_range,
            optional: false,
        }
    }

    /// `"id"` or `"id@range"`.
    ///
    /// # Errors
    /// Invalid range.
    pub fn parse(s: &str) -> AppResult<Self> {
        let (id, range) = match s.split_once('@') {
            Some((id, r)) => (id, VersionRange::parse(r.trim())?),
            None => (s, VersionRange::default()),
        };
        Ok(Self::new(id.trim(), range))
    }
}

impl fmt::Display for PluginDependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.plugin_id, self.version_range)
    }
}

/// C# `DependencyConflict`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyConflict {
    pub dependency_id: String,
    pub required_by_1: String,
    pub requirement_1: VersionRange,
    pub required_by_2: String,
    pub requirement_2: VersionRange,
    pub available_version: Option<SemanticVersion>,
}

/// C# `DependencyResolutionResult`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DependencyResolutionResult {
    pub success: bool,
    pub resolved_order: Vec<String>,
    pub conflicts: Vec<DependencyConflict>,
    pub missing_dependencies: Vec<String>,
}

impl DependencyResolutionResult {
    #[must_use]
    pub fn succeeded(order: Vec<String>) -> Self {
        Self {
            success: true,
            resolved_order: order,
            ..Self::default()
        }
    }
    #[must_use]
    pub fn failed(conflicts: Vec<DependencyConflict>, missing: Vec<String>) -> Self {
        Self {
            success: false,
            resolved_order: Vec::new(),
            conflicts,
            missing_dependencies: missing,
        }
    }
}
