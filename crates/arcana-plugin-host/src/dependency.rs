//! C# `DependencyResolver`: missing / version-conflict detection and topological load order.

use std::collections::HashSet;

use arcana_core::AppResult;
use arcana_plugin_api::PluginMetadata;

use crate::version::{
    DependencyConflict, DependencyResolutionResult, PluginDependency, SemanticVersion, VersionRange,
};

#[derive(Debug, Clone)]
struct Info {
    id: String,
    version: SemanticVersion,
    dependencies: Vec<PluginDependency>,
}

/// C# `DependencyResolver`. Plugins keep registration order (the C# `Dictionary`
/// enumeration order), which decides the order of independent plugins in the result.
#[derive(Debug, Clone, Default)]
pub struct DependencyResolver {
    plugins: Vec<Info>,
}

impl DependencyResolver {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn get(&self, id: &str) -> Option<&Info> {
        self.plugins.iter().find(|p| p.id == id)
    }

    /// C# `Register(id, version, deps)`; re-registering replaces in place.
    pub fn register(
        &mut self,
        plugin_id: &str,
        version: SemanticVersion,
        dependencies: Vec<PluginDependency>,
    ) {
        let info = Info {
            id: plugin_id.to_string(),
            version,
            dependencies,
        };
        match self.plugins.iter_mut().find(|p| p.id == plugin_id) {
            Some(slot) => *slot = info,
            None => self.plugins.push(info),
        }
    }

    /// C# `Register(PluginMetadata)`.
    ///
    /// # Errors
    /// A dependency string with an invalid range.
    pub fn register_metadata(&mut self, metadata: &PluginMetadata) -> AppResult<()> {
        let deps = metadata
            .dependencies
            .iter()
            .map(|d| PluginDependency::parse(d))
            .collect::<AppResult<Vec<_>>>()?;
        self.register(
            &metadata.id,
            SemanticVersion::from_version(&metadata.version),
            deps,
        );
        Ok(())
    }

    /// C# `Resolve`.
    #[must_use]
    pub fn resolve(&self) -> DependencyResolutionResult {
        let mut conflicts = Vec::new();
        let mut missing = Vec::new();
        for info in &self.plugins {
            for dep in &info.dependencies {
                let Some(target) = self.get(&dep.plugin_id) else {
                    if !dep.optional {
                        missing.push(format!("{} requires {}", info.id, dep.plugin_id));
                    }
                    continue;
                };
                if !dep.version_range.is_satisfied_by(&target.version) {
                    conflicts.push(DependencyConflict {
                        dependency_id: dep.plugin_id.clone(),
                        required_by_1: info.id.clone(),
                        requirement_1: dep.version_range.clone(),
                        required_by_2: dep.plugin_id.clone(),
                        requirement_2: VersionRange::exact(target.version.clone()),
                        available_version: Some(target.version.clone()),
                    });
                }
            }
        }
        // Second pass (C# "conflicting version requirements"): every unsatisfied requirement
        // again, paired with a satisfied one when there is any. C# reports each twice too.
        let mut requirements: Vec<(String, Vec<(String, VersionRange)>)> = Vec::new();
        for info in &self.plugins {
            for dep in &info.dependencies {
                let req = (info.id.clone(), dep.version_range.clone());
                if let Some((_, list)) = requirements.iter_mut().find(|(d, _)| *d == dep.plugin_id)
                {
                    list.push(req);
                } else {
                    requirements.push((dep.plugin_id.clone(), vec![req]));
                }
            }
        }
        for (dep_id, reqs) in &requirements {
            let Some(target) = self.get(dep_id) else {
                continue;
            };
            let satisfied = reqs
                .iter()
                .find(|(_, r)| r.is_satisfied_by(&target.version));
            for (by, range) in reqs
                .iter()
                .filter(|(_, r)| !r.is_satisfied_by(&target.version))
            {
                let (by2, range2) = satisfied.map_or_else(
                    || {
                        (
                            dep_id.clone(),
                            VersionRange {
                                min_version: Some(target.version.clone()),
                                max_version: Some(target.version.clone()),
                                ..VersionRange::default()
                            },
                        )
                    },
                    |(b, r)| (b.clone(), r.clone()),
                );
                conflicts.push(DependencyConflict {
                    dependency_id: dep_id.clone(),
                    required_by_1: by.clone(),
                    requirement_1: range.clone(),
                    required_by_2: by2,
                    requirement_2: range2,
                    available_version: Some(target.version.clone()),
                });
            }
        }
        if !conflicts.is_empty() || !missing.is_empty() {
            for c in &conflicts {
                tracing::error!(
                    dependency = %c.dependency_id, by1 = %c.required_by_1, range1 = %c.requirement_1,
                    by2 = %c.required_by_2, range2 = %c.requirement_2, "dependency conflict"
                );
            }
            for m in &missing {
                tracing::error!(missing = %m, "missing dependency");
            }
            return DependencyResolutionResult::failed(conflicts, missing);
        }
        if let Some(order) = self.topological_sort() {
            DependencyResolutionResult::succeeded(order)
        } else {
            tracing::error!("circular dependency detected");
            DependencyResolutionResult::failed(Vec::new(), Vec::new())
        }
    }

    /// C# `ResolveFor`: only `plugin_id` and its non-optional transitive dependencies.
    #[must_use]
    pub fn resolve_for(&self, plugin_id: &str) -> DependencyResolutionResult {
        if self.get(plugin_id).is_none() {
            return DependencyResolutionResult::failed(Vec::new(), vec![plugin_id.to_string()]);
        }
        let mut required = HashSet::new();
        self.collect(plugin_id, &mut required);
        let mut sub = Self::new();
        for p in self.plugins.iter().filter(|p| required.contains(&p.id)) {
            sub.register(&p.id, p.version.clone(), p.dependencies.clone());
        }
        sub.resolve()
    }

    fn collect(&self, id: &str, out: &mut HashSet<String>) {
        if !out.insert(id.to_string()) {
            return;
        }
        if let Some(info) = self.get(id) {
            for d in info.dependencies.iter().filter(|d| !d.optional) {
                self.collect(&d.plugin_id, out);
            }
        }
    }

    /// `None` on a cycle (C# throws `InvalidOperationException`).
    fn topological_sort(&self) -> Option<Vec<String>> {
        let mut sorted = Vec::new();
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();
        for p in &self.plugins {
            self.visit(&p.id, &mut visited, &mut visiting, &mut sorted)?;
        }
        Some(sorted)
    }

    fn visit(
        &self,
        id: &str,
        visited: &mut HashSet<String>,
        visiting: &mut HashSet<String>,
        sorted: &mut Vec<String>,
    ) -> Option<()> {
        if visited.contains(id) {
            return Some(());
        }
        if !visiting.insert(id.to_string()) {
            return None;
        }
        if let Some(info) = self.get(id) {
            for d in &info.dependencies {
                if self.get(&d.plugin_id).is_some() {
                    self.visit(&d.plugin_id, visited, visiting, sorted)?;
                }
            }
        }
        visiting.remove(id);
        visited.insert(id.to_string());
        sorted.push(id.to_string());
        Some(())
    }
}
