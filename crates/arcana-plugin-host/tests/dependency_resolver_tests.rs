//! C# `DependencyResolverTests` (15).
#![allow(clippy::unwrap_used)]

use arcana_plugin_host::dependency::DependencyResolver;
use arcana_plugin_host::version::{PluginDependency, SemanticVersion, VersionRange};

fn v(s: &str) -> SemanticVersion {
    SemanticVersion::parse(s).unwrap()
}
fn d(s: &str) -> PluginDependency {
    PluginDependency::parse(s).unwrap()
}
fn idx(order: &[String], id: &str) -> usize {
    order.iter().position(|x| x == id).unwrap()
}

/// `Resolve_NoDependencies_ShouldSucceed`
#[test]
fn resolve_no_dependencies_should_succeed() {
    let mut r = DependencyResolver::new();
    r.register("plugin-a", v("1.0.0"), vec![]);
    r.register("plugin-b", v("1.0.0"), vec![]);
    let res = r.resolve();
    assert!(res.success);
    assert!(res.resolved_order.contains(&"plugin-a".into()));
    assert!(res.resolved_order.contains(&"plugin-b".into()));
}

/// `Resolve_SimpleDependency_ShouldOrderCorrectly`
#[test]
fn resolve_simple_dependency_should_order_correctly() {
    let mut r = DependencyResolver::new();
    r.register("plugin-a", v("1.0.0"), vec![d("plugin-b@^1.0.0")]);
    r.register("plugin-b", v("1.0.0"), vec![]);
    let res = r.resolve();
    assert!(res.success);
    assert!(idx(&res.resolved_order, "plugin-b") < idx(&res.resolved_order, "plugin-a"));
}

/// `Resolve_ChainedDependencies_ShouldOrderCorrectly`
#[test]
fn resolve_chained_dependencies_should_order_correctly() {
    let mut r = DependencyResolver::new();
    r.register("plugin-a", v("1.0.0"), vec![]);
    r.register("plugin-b", v("1.0.0"), vec![d("plugin-a@^1.0.0")]);
    r.register("plugin-c", v("1.0.0"), vec![d("plugin-b@^1.0.0")]);
    let res = r.resolve();
    assert!(res.success);
    let o = &res.resolved_order;
    assert!(idx(o, "plugin-a") < idx(o, "plugin-b"));
    assert!(idx(o, "plugin-b") < idx(o, "plugin-c"));
}

/// `Resolve_DiamondDependency_ShouldSucceed`
#[test]
fn resolve_diamond_dependency_should_succeed() {
    let mut r = DependencyResolver::new();
    r.register("plugin-a", v("1.0.0"), vec![]);
    r.register("plugin-b", v("1.0.0"), vec![d("plugin-a@^1.0.0")]);
    r.register("plugin-c", v("1.0.0"), vec![d("plugin-a@^1.0.0")]);
    r.register(
        "plugin-d",
        v("1.0.0"),
        vec![d("plugin-b@^1.0.0"), d("plugin-c@^1.0.0")],
    );
    let res = r.resolve();
    assert!(res.success);
    let o = &res.resolved_order;
    assert!(idx(o, "plugin-a") < idx(o, "plugin-b"));
    assert!(idx(o, "plugin-a") < idx(o, "plugin-c"));
    assert!(idx(o, "plugin-b") < idx(o, "plugin-d"));
    assert!(idx(o, "plugin-c") < idx(o, "plugin-d"));
}

/// `Resolve_SatisfiedVersionConstraint_ShouldSucceed`
#[test]
fn resolve_satisfied_version_constraint_should_succeed() {
    let mut r = DependencyResolver::new();
    r.register("plugin-a", v("1.0.0"), vec![d("plugin-b@>=1.0.0")]);
    r.register("plugin-b", v("1.5.0"), vec![]);
    assert!(r.resolve().success);
}

/// `Resolve_UnsatisfiedVersionConstraint_ShouldFail`
#[test]
fn resolve_unsatisfied_version_constraint_should_fail() {
    let mut r = DependencyResolver::new();
    r.register("plugin-a", v("1.0.0"), vec![d("plugin-b@>=2.0.0")]);
    r.register("plugin-b", v("1.5.0"), vec![]);
    let res = r.resolve();
    assert!(!res.success);
    assert!(!res.conflicts.is_empty());
}

/// `Resolve_ConflictingVersionRequirements_ShouldFail`
#[test]
fn resolve_conflicting_version_requirements_should_fail() {
    let mut r = DependencyResolver::new();
    r.register("plugin-a", v("1.0.0"), vec![d("plugin-c@>=2.0.0")]);
    r.register("plugin-b", v("1.0.0"), vec![d("plugin-c@<2.0.0")]);
    r.register("plugin-c", v("1.5.0"), vec![]);
    let res = r.resolve();
    assert!(!res.success);
    assert!(!res.conflicts.is_empty());
}

/// `Resolve_CaretVersionConstraint_ShouldAllowMinorUpdates`
#[test]
fn resolve_caret_version_constraint_should_allow_minor_updates() {
    let mut r = DependencyResolver::new();
    r.register("plugin-a", v("1.0.0"), vec![d("plugin-b@^1.0.0")]);
    r.register("plugin-b", v("1.9.9"), vec![]);
    assert!(r.resolve().success);
}

/// `Resolve_CaretVersionConstraint_ShouldRejectMajorUpdates`
#[test]
fn resolve_caret_version_constraint_should_reject_major_updates() {
    let mut r = DependencyResolver::new();
    r.register("plugin-a", v("1.0.0"), vec![d("plugin-b@^1.0.0")]);
    r.register("plugin-b", v("2.0.0"), vec![]);
    let res = r.resolve();
    assert!(!res.success);
    assert!(!res.conflicts.is_empty());
}

/// `Resolve_MissingDependency_ShouldFail`
#[test]
fn resolve_missing_dependency_should_fail() {
    let mut r = DependencyResolver::new();
    r.register("plugin-a", v("1.0.0"), vec![d("plugin-b@^1.0.0")]);
    let res = r.resolve();
    assert!(!res.success);
    assert!(!res.missing_dependencies.is_empty());
}

/// `Resolve_OptionalMissingDependency_ShouldSucceed`
#[test]
fn resolve_optional_missing_dependency_should_succeed() {
    let mut r = DependencyResolver::new();
    let mut dep = PluginDependency::new("plugin-b", VersionRange::parse("^1.0.0").unwrap());
    dep.optional = true;
    r.register("plugin-a", v("1.0.0"), vec![dep]);
    assert!(r.resolve().success);
}

/// `Resolve_CircularDependency_ShouldFail`
#[test]
fn resolve_circular_dependency_should_fail() {
    let mut r = DependencyResolver::new();
    r.register("plugin-a", v("1.0.0"), vec![d("plugin-b@^1.0.0")]);
    r.register("plugin-b", v("1.0.0"), vec![d("plugin-a@^1.0.0")]);
    assert!(!r.resolve().success);
}

/// `Resolve_IndirectCircularDependency_ShouldFail`
#[test]
fn resolve_indirect_circular_dependency_should_fail() {
    let mut r = DependencyResolver::new();
    r.register("plugin-a", v("1.0.0"), vec![d("plugin-b@^1.0.0")]);
    r.register("plugin-b", v("1.0.0"), vec![d("plugin-c@^1.0.0")]);
    r.register("plugin-c", v("1.0.0"), vec![d("plugin-a@^1.0.0")]);
    assert!(!r.resolve().success);
}

/// `ResolveFor_SpecificPlugin_ShouldReturnOnlyRequiredDependencies`
#[test]
fn resolve_for_specific_plugin_should_return_only_required_dependencies() {
    let mut r = DependencyResolver::new();
    r.register("plugin-a", v("1.0.0"), vec![]);
    r.register("plugin-b", v("1.0.0"), vec![d("plugin-a@^1.0.0")]);
    r.register("plugin-c", v("1.0.0"), vec![]);
    let res = r.resolve_for("plugin-b");
    assert!(res.success);
    assert!(res.resolved_order.contains(&"plugin-a".into()));
    assert!(res.resolved_order.contains(&"plugin-b".into()));
    assert!(!res.resolved_order.contains(&"plugin-c".into()));
}

/// `ResolveFor_UnknownPlugin_ShouldFail`
#[test]
fn resolve_for_unknown_plugin_should_fail() {
    let mut r = DependencyResolver::new();
    r.register("plugin-a", v("1.0.0"), vec![]);
    let res = r.resolve_for("plugin-unknown");
    assert!(!res.success);
    assert!(res.missing_dependencies.contains(&"plugin-unknown".into()));
}

/// Rust addition: `register_metadata` parses `id@range` strings from `PluginMetadata`.
#[test]
fn register_metadata_parses_dependency_strings() {
    use arcana_plugin_api::{PluginMetadata, PluginType};
    let mut r = DependencyResolver::new();
    let mut a = PluginMetadata::new("a", "A", semver::Version::new(1, 0, 0), PluginType::Module);
    a.dependencies = vec!["b@^2.0.0".into()];
    let b = PluginMetadata::new("b", "B", semver::Version::new(2, 1, 0), PluginType::Module);
    r.register_metadata(&a).unwrap();
    r.register_metadata(&b).unwrap();
    assert_eq!(r.resolve().resolved_order, ["b", "a"]);
}
