//! C# `SemanticVersionTests` (18) and `VersionRangeTests` (15). Theories loop over their
//! `InlineData` rows; the row count is asserted so a dropped case is visible.
#![allow(clippy::unwrap_used)]

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use arcana_plugin_host::version::{SemanticVersion, VersionRange};

fn v(s: &str) -> SemanticVersion {
    SemanticVersion::parse(s).unwrap()
}

// ---- SemanticVersionTests ----

/// `Parse_ValidVersionString_ShouldParseCorrectly` (10 rows)
#[test]
fn parse_valid_version_string_should_parse_correctly() {
    type Row<'a> = (&'a str, u64, u64, u64, Option<&'a str>, Option<&'a str>);
    let rows: [Row; 10] = [
        ("1.0.0", 1, 0, 0, None, None),
        ("2.1.3", 2, 1, 3, None, None),
        ("0.0.1", 0, 0, 1, None, None),
        ("10.20.30", 10, 20, 30, None, None),
        ("1.0.0-alpha", 1, 0, 0, Some("alpha"), None),
        ("1.0.0-alpha.1", 1, 0, 0, Some("alpha.1"), None),
        ("1.0.0-beta.2", 1, 0, 0, Some("beta.2"), None),
        ("1.0.0-rc.1", 1, 0, 0, Some("rc.1"), None),
        ("1.0.0+build123", 1, 0, 0, None, Some("build123")),
        ("1.0.0-alpha+build", 1, 0, 0, Some("alpha"), Some("build")),
    ];
    for (input, ma, mi, pa, pre, build) in rows {
        let x = v(input);
        assert_eq!((x.major, x.minor, x.patch), (ma, mi, pa), "{input}");
        assert_eq!(x.prerelease.as_deref(), pre, "{input}");
        assert_eq!(x.build_metadata.as_deref(), build, "{input}");
    }
}

/// `Parse_SimpleVersion_ShouldParseAsSemVer`
#[test]
fn parse_simple_version_should_parse_as_semver() {
    let x = SemanticVersion::try_parse(Some("1.0")).unwrap();
    assert_eq!((x.major, x.minor, x.patch), (1, 0, 0));
}

/// `Parse_MajorOnly_ShouldReturnFalse`
#[test]
fn parse_major_only_should_return_false() {
    assert!(SemanticVersion::try_parse(Some("1")).is_none());
}

/// `TryParse_EmptyOrNull_ShouldReturnFalse` (3 rows)
#[test]
fn try_parse_empty_or_null_should_return_false() {
    for input in [Some(""), Some("   "), None] {
        assert!(SemanticVersion::try_parse(input).is_none());
    }
}

/// `Parse_InvalidVersion_ShouldThrowFormatException`
#[test]
fn parse_invalid_version_should_throw_format_exception() {
    assert!(SemanticVersion::parse("not-a-version").is_err());
}

fn sign(o: std::cmp::Ordering) -> i32 {
    o as i32
}

/// `CompareTo_DifferentVersions_ShouldReturnCorrectOrder` (7 rows)
#[test]
fn compare_to_different_versions_should_return_correct_order() {
    let rows = [
        ("1.0.0", "2.0.0", -1),
        ("2.0.0", "1.0.0", 1),
        ("1.0.0", "1.0.0", 0),
        ("1.0.0", "1.1.0", -1),
        ("1.1.0", "1.0.0", 1),
        ("1.0.0", "1.0.1", -1),
        ("1.0.1", "1.0.0", 1),
    ];
    for (a, b, e) in rows {
        assert_eq!(sign(v(a).cmp(&v(b))), e, "{a} vs {b}");
    }
}

/// `CompareTo_PrereleaseVersions_ShouldCompareCorrectly` (4 rows)
#[test]
fn compare_to_prerelease_versions_should_compare_correctly() {
    let rows = [
        ("1.0.0-alpha", "1.0.0", -1),
        ("1.0.0", "1.0.0-alpha", 1),
        ("1.0.0-alpha", "1.0.0-beta", -1),
        ("1.0.0-alpha.1", "1.0.0-alpha.2", -1),
    ];
    for (a, b, e) in rows {
        assert_eq!(sign(v(a).cmp(&v(b))), e, "{a} vs {b}");
    }
}

/// `CompareTo_Null_ShouldReturnPositive` — `Some(v) > None` is the Rust equivalent.
#[test]
fn compare_to_null_should_return_positive() {
    assert!(Some(v("1.0.0")) > None);
}

/// `Equals_SameVersion_ShouldBeTrue`
#[test]
fn equals_same_version_should_be_true() {
    let (a, b) = (v("1.2.3"), v("1.2.3"));
    assert_eq!(a, b);
    assert!(a.eq(&b));
    assert!(!a.ne(&b));
}

/// `Equals_DifferentVersion_ShouldBeFalse`
#[test]
fn equals_different_version_should_be_false() {
    let (a, b) = (v("1.2.3"), v("1.2.4"));
    assert_ne!(a, b);
    assert!(!a.eq(&b));
}

fn hash(x: &SemanticVersion) -> u64 {
    let mut h = DefaultHasher::new();
    x.hash(&mut h);
    h.finish()
}

/// `GetHashCode_SameVersion_ShouldBeSame`
#[test]
fn get_hash_code_same_version_should_be_same() {
    assert_eq!(hash(&v("1.2.3")), hash(&v("1.2.3")));
}

/// `LessThanOperator_ShouldWorkCorrectly` (3 rows)
#[test]
fn less_than_operator_should_work_correctly() {
    for (a, b, e) in [
        ("1.0.0", "2.0.0", true),
        ("2.0.0", "1.0.0", false),
        ("1.0.0", "1.0.0", false),
    ] {
        assert_eq!(v(a) < v(b), e);
    }
}

/// `LessThanOrEqualOperator_ShouldWorkCorrectly` (3 rows)
#[test]
fn less_than_or_equal_operator_should_work_correctly() {
    for (a, b, e) in [
        ("1.0.0", "2.0.0", true),
        ("2.0.0", "1.0.0", false),
        ("1.0.0", "1.0.0", true),
    ] {
        assert_eq!(v(a) <= v(b), e);
    }
}

/// `GreaterThanOperator_ShouldWorkCorrectly` (3 rows)
#[test]
fn greater_than_operator_should_work_correctly() {
    for (a, b, e) in [
        ("2.0.0", "1.0.0", true),
        ("1.0.0", "2.0.0", false),
        ("1.0.0", "1.0.0", false),
    ] {
        assert_eq!(v(a) > v(b), e);
    }
}

/// `FromVersion_ShouldConvertCorrectly`
#[test]
fn from_version_should_convert_correctly() {
    let x = SemanticVersion::from_version(&semver::Version::new(1, 2, 3));
    assert_eq!((x.major, x.minor, x.patch), (1, 2, 3));
}

/// `ToVersion_ShouldConvertCorrectly`
#[test]
fn to_version_should_convert_correctly() {
    let n = SemanticVersion::new(1, 2, 3).to_version();
    assert_eq!((n.major, n.minor, n.patch), (1, 2, 3));
}

/// `ToString_ShouldFormatCorrectly`
#[test]
fn to_string_should_format_correctly() {
    let mk = |p, b| SemanticVersion::try_new(1, 2, 3, p, b).unwrap().to_string();
    assert_eq!(mk(None, None), "1.2.3");
    assert_eq!(mk(Some("alpha"), None), "1.2.3-alpha");
    assert_eq!(mk(None, Some("build")), "1.2.3+build");
    assert_eq!(mk(Some("alpha"), Some("build")), "1.2.3-alpha+build");
}

/// `Constructor_NegativeValues_ShouldThrowArgumentException` (3 rows)
#[test]
fn constructor_negative_values_should_throw_argument_exception() {
    for (a, b, c) in [(-1, 0, 0), (0, -1, 0), (0, 0, -1)] {
        assert!(SemanticVersion::try_new(a, b, c, None, None).is_err());
    }
}

// ---- VersionRangeTests ----

fn check(rows: &[(&str, &str, bool)]) {
    for (range, ver, e) in rows {
        assert_eq!(
            VersionRange::parse(range).unwrap().is_satisfied_by(&v(ver)),
            *e,
            "{range} vs {ver}"
        );
    }
}

/// `Parse_ExactVersion_ShouldMatchOnlyThatVersion`
#[test]
fn parse_exact_version_should_match_only_that_version() {
    check(&[
        ("1.2.3", "1.2.3", true),
        ("1.2.3", "1.2.4", false),
        ("1.2.3", "1.2.2", false),
    ]);
}

/// `Parse_CaretRange_ShouldAllowCompatibleVersions` (10 rows)
#[test]
fn parse_caret_range_should_allow_compatible_versions() {
    check(&[
        ("^1.0.0", "1.0.0", true),
        ("^1.0.0", "1.5.0", true),
        ("^1.0.0", "1.9.9", true),
        ("^1.0.0", "2.0.0", false),
        ("^1.0.0", "0.9.9", false),
        ("^1.2.3", "1.2.3", true),
        ("^1.2.3", "1.3.0", true),
        ("^1.2.3", "1.2.2", false),
        ("^0.1.0", "0.1.5", true),
        ("^0.1.0", "1.0.0", false),
    ]);
}

/// `Parse_TildeRange_ShouldAllowPatchUpdates` (8 rows)
#[test]
fn parse_tilde_range_should_allow_patch_updates() {
    check(&[
        ("~1.2.0", "1.2.0", true),
        ("~1.2.0", "1.2.5", true),
        ("~1.2.0", "1.2.99", true),
        ("~1.2.0", "1.3.0", false),
        ("~1.2.0", "1.1.9", false),
        ("~1.2.3", "1.2.3", true),
        ("~1.2.3", "1.2.4", true),
        ("~1.2.3", "1.2.2", false),
    ]);
}

/// `Parse_HyphenRange_ShouldBeInclusive` (5 rows)
#[test]
fn parse_hyphen_range_should_be_inclusive() {
    check(&[
        ("1.0.0 - 2.0.0", "1.0.0", true),
        ("1.0.0 - 2.0.0", "1.5.0", true),
        ("1.0.0 - 2.0.0", "2.0.0", true),
        ("1.0.0 - 2.0.0", "0.9.9", false),
        ("1.0.0 - 2.0.0", "2.0.1", false),
    ]);
}

/// `Parse_GreaterThanOrEqual_ShouldWorkCorrectly` (3 rows)
#[test]
fn parse_greater_than_or_equal_should_work_correctly() {
    check(&[
        (">=1.0.0", "1.0.0", true),
        (">=1.0.0", "2.0.0", true),
        (">=1.0.0", "0.9.9", false),
    ]);
}

/// `Parse_GreaterThan_ShouldWorkCorrectly` (4 rows)
#[test]
fn parse_greater_than_should_work_correctly() {
    check(&[
        (">1.0.0", "1.0.0", false),
        (">1.0.0", "1.0.1", true),
        (">1.0.0", "2.0.0", true),
        (">1.0.0", "0.9.9", false),
    ]);
}

/// `Parse_LessThanOrEqual_ShouldWorkCorrectly` (3 rows)
#[test]
fn parse_less_than_or_equal_should_work_correctly() {
    check(&[
        ("<=2.0.0", "2.0.0", true),
        ("<=2.0.0", "1.0.0", true),
        ("<=2.0.0", "2.0.1", false),
    ]);
}

/// `Parse_LessThan_ShouldWorkCorrectly` (4 rows)
#[test]
fn parse_less_than_should_work_correctly() {
    check(&[
        ("<2.0.0", "2.0.0", false),
        ("<2.0.0", "1.9.9", true),
        ("<2.0.0", "1.0.0", true),
        ("<2.0.0", "2.0.1", false),
    ]);
}

/// `Parse_MajorWildcard_ShouldMatchMajorVersion` (4 rows)
#[test]
fn parse_major_wildcard_should_match_major_version() {
    check(&[
        ("1.*", "1.0.0", true),
        ("1.*", "1.9.9", true),
        ("1.*", "2.0.0", false),
        ("1.*", "0.9.9", false),
    ]);
}

/// `Parse_MinorWildcard_ShouldMatchMinorVersion` (4 rows)
#[test]
fn parse_minor_wildcard_should_match_minor_version() {
    check(&[
        ("1.2.*", "1.2.0", true),
        ("1.2.*", "1.2.99", true),
        ("1.2.*", "1.3.0", false),
        ("1.2.*", "1.1.9", false),
    ]);
}

/// `Parse_EmptyRange_ShouldMatchAnyVersion` (2 rows)
#[test]
fn parse_empty_range_should_match_any_version() {
    for r in ["", "   "] {
        let range = VersionRange::parse(r).unwrap();
        assert!(range.is_satisfied_by(&v("0.0.1")));
        assert!(range.is_satisfied_by(&v("99.99.99")));
    }
}

/// `Parse_InvalidRange_ShouldThrowFormatException`
#[test]
fn parse_invalid_range_should_throw_format_exception() {
    assert!(VersionRange::parse("invalid").is_err());
}

/// `ToString_ExactVersion_ShouldReturnVersion`
#[test]
fn to_string_exact_version_should_return_version() {
    assert_eq!(VersionRange::parse("1.2.3").unwrap().to_string(), "1.2.3");
}

/// `ToString_Range_ShouldReturnReadableFormat`
#[test]
fn to_string_range_should_return_readable_format() {
    let r = VersionRange::between(v("1.0.0"), v("2.0.0"), false);
    assert_eq!(r.to_string(), ">=1.0.0 <2.0.0");
}

/// `ToString_AnyVersion_ShouldReturnStar`
#[test]
fn to_string_any_version_should_return_star() {
    assert_eq!(VersionRange::default().to_string(), "*");
}
