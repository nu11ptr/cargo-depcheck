use crate::{dep_tree::Deps, Package};

use cargo_lock::{Name, Version};
use indexmap::{IndexMap, IndexSet};

// *** TopLevelPackages ***

#[derive(Default)]
pub(crate) struct TopLevelPackages(IndexSet<Package>);

impl TopLevelPackages {
    pub fn add(&mut self, top_level: Package) {
        self.0.insert(top_level);
    }
}

impl std::fmt::Display for TopLevelPackages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for pkg in &self.0 {
            writeln!(f, "          {pkg}")?;
        }

        Ok(())
    }
}

// *** TopLevelDependencies ***

#[derive(Default)]
pub(crate) struct TopLevelDependencies(IndexMap<Package, TopLevelPackages>);

impl TopLevelDependencies {
    pub fn add(&mut self, top_level_dep: Package) -> &mut TopLevelPackages {
        self.0.entry(top_level_dep).or_default()
    }
}

impl std::fmt::Display for TopLevelDependencies {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (direct, top_level) in self.0.iter() {
            writeln!(f, "        {direct}")?;
            writeln!(f, "{top_level}")?;
        }

        Ok(())
    }
}

// *** DependencyParents ***

#[derive(Default)]
pub(crate) struct DependencyParents(IndexMap<Package, TopLevelDependencies>);

impl DependencyParents {
    pub fn add(&mut self, direct: Package) -> &mut TopLevelDependencies {
        self.0.entry(direct).or_default()
    }
}

impl std::fmt::Display for DependencyParents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (direct, top_level_deps) in self.0.iter() {
            writeln!(f, "      {direct}")?;
            write!(f, "{top_level_deps}")?;
        }

        Ok(())
    }
}

// *** MultiVerDep ***

/// Represents a dependency that has multiple versions. It can track 3 levels of hierarchy:
/// the direct dependent, the top level's dependencies, and the top level dependents. It intentionally
/// skips the levels between the direct dependent and the top level dependents for brevity.
pub(crate) struct MultiVerDep {
    versions: IndexMap<Version, DependencyParents>,
}

impl MultiVerDep {
    pub fn new(versions: IndexSet<Version>) -> Self {
        Self {
            versions: versions
                .into_iter()
                .map(|ver| (ver, DependencyParents::default()))
                .collect(),
        }
    }

    pub fn version_keys(&self) -> impl Iterator<Item = &Version> {
        self.versions.keys()
    }

    pub fn versions_mut(&mut self) -> impl Iterator<Item = (&Version, &mut DependencyParents)> {
        self.versions.iter_mut()
    }
}

impl std::fmt::Display for MultiVerDep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (version, direct_and_top_level) in self.versions.iter() {
            writeln!(f, "    {version}:")?;
            write!(f, "{direct_and_top_level}")?;
        }

        Ok(())
    }
}

// *** MultiVerDeps ***
pub(crate) struct MultiVerDeps(IndexMap<Name, MultiVerDep>);

impl MultiVerDeps {
    pub fn from_deps(deps: &Deps) -> Self {
        let multi_ver_deps: IndexMap<_, _> = deps
            .iter()
            .filter_map(|(name, dep)| {
                if dep.has_multiple_versions() {
                    Some((name.clone(), MultiVerDep::new(dep.versions())))
                } else {
                    None
                }
            })
            .collect();

        Self(multi_ver_deps)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Name, &MultiVerDep)> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&Name, &mut MultiVerDep)> {
        self.0.iter_mut()
    }

    pub fn sort(&mut self) {
        self.0.sort_unstable_keys();
    }
}

impl std::fmt::Display for MultiVerDeps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (name, multi_ver_dep) in &self.0 {
            writeln!(f, "{name}:")?;
            writeln!(f, "{multi_ver_dep}")?;
        }

        Ok(())
    }
}
