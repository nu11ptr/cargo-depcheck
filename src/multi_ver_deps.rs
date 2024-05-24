use crate::dep_tree::Deps;

use cargo_lock::{Name, Version};
use indexmap::{IndexMap, IndexSet};

// *** MultiVerDep ***

/// Represents a dependency that has multiple versions. It can track 3 levels of hierarchy:
/// the direct dependent, the top level's dependencies, and the top level dependents. It intentionally
/// skips the levels between the direct dependent and the top level dependents for brevity.
pub(crate) struct MultiVerDep(IndexSet<Version>);

impl MultiVerDep {
    pub fn new(versions: IndexSet<Version>) -> Self {
        Self(versions)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Version> {
        self.0.iter()
    }

    pub fn ver_count(&self) -> usize {
        self.0.len()
    }
}

impl std::fmt::Display for MultiVerDep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let versions = self
            .0
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        f.write_str(&versions)
    }
}

// *** MultiVerDeps ***

pub struct MultiVerDeps(IndexMap<Name, MultiVerDep>);

impl MultiVerDeps {
    pub fn from_deps(deps: &Deps) -> Self {
        let mut multi_ver_deps: IndexMap<_, _> = deps
            .iter()
            .filter_map(|(name, dep)| {
                if dep.has_multiple_versions() {
                    Some((name.clone(), MultiVerDep::new(dep.versions())))
                } else {
                    None
                }
            })
            .collect();

        multi_ver_deps.sort_unstable_keys();
        Self(multi_ver_deps)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn dup_pkg_count(&self) -> usize {
        self.0.len()
    }

    pub fn dup_ver_count(&self) -> usize {
        self.0.values().map(|mv_dep| mv_dep.ver_count()).sum()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&Name, &MultiVerDep)> {
        self.0.iter()
    }
}

impl std::fmt::Display for MultiVerDeps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (name, multi_ver_dep) in &self.0 {
            writeln!(f, "{name} ({multi_ver_dep})")?;
        }

        Ok(())
    }
}
