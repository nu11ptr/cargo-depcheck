use std::hash::Hash;

use cargo_lock::{Dependency, Lockfile, Name, ResolveVersion, Version};
use indexmap::{IndexMap, IndexSet};

use crate::{
    multi_ver_deps::MultiVerDep, multi_ver_parents::MultiVerParents, results::DupDepResults,
};

// *** Deps ***

#[derive(Debug)]
pub struct Deps {
    deps: IndexMap<Name, Dep>,
}

impl Deps {
    pub fn from_lock_file(lock_file: Lockfile) -> Result<Self, String> {
        let mut deps = IndexMap::with_capacity(lock_file.packages.len());

        // I can't find any examples of non-v3 lock files, so I'm not sure if this is necessary
        if lock_file.version != ResolveVersion::V3 {
            return Err("Only v3 lock files are supported".into());
        }

        for package in lock_file.packages {
            let top_level = package.source.is_none();

            let dep = deps
                .entry(package.name.clone())
                .or_insert_with(|| Dep::new(package.name.clone()));
            dep.add_modify_ver_dependencies(
                package.version.clone(),
                top_level,
                &package.dependencies,
            );

            // Add all dependents
            for dependency in package.dependencies {
                let dependent = Package {
                    name: package.name.clone(),
                    version: package.version.clone(),
                };

                let top_level = dependency.source.is_none();

                let dep = deps
                    .entry(dependency.name.clone())
                    .or_insert_with(|| Dep::new(dependency.name.clone()));
                dep.add_modify_ver_dependent(dependency.version, top_level, dependent);
            }
        }

        Ok(Deps { deps })
    }

    pub fn get_version(&self, pkg: &Package) -> Result<&DepVersion, String> {
        let dep = self.deps.get(&pkg.name).ok_or(format!(
            "Corrupted lock file: Dependency '{}' not found",
            pkg.name
        ))?;

        dep.versions.get(&pkg.version).ok_or(format!(
            "Corrupted lock file: Version '{}' of '{}' not found",
            pkg.version, pkg.name
        ))
    }

    pub fn build_dup_dep_results(
        &self,
        show_deps: bool,
        show_dups: bool,
        verbose: bool,
    ) -> Result<DupDepResults, String> {
        let multi_ver_deps: IndexMap<_, _> = self
            .deps
            .values()
            .filter_map(|dep| {
                if dep.has_multiple_versions() {
                    Some((
                        dep.name.clone(),
                        MultiVerDep::new(dep.name.clone(), dep.versions()),
                    ))
                } else {
                    None
                }
            })
            .collect();

        let multi_ver_parents = MultiVerParents::build(self, &multi_ver_deps)?;

        DupDepResults::from_multi_ver_deps_parents(
            multi_ver_deps,
            &multi_ver_parents,
            show_deps,
            show_dups,
            verbose,
            self,
        )
    }
}

// *** Dep ***

#[derive(Debug)]
struct Dep {
    name: Name,
    versions: IndexMap<Version, DepVersion>,
}

impl Dep {
    pub fn new(name: Name) -> Self {
        Self {
            name,
            versions: IndexMap::new(),
        }
    }

    pub fn has_multiple_versions(&self) -> bool {
        self.versions.len() > 1
    }

    pub fn versions(&self) -> IndexSet<Version> {
        self.versions.keys().cloned().collect()
    }

    pub fn add_modify_ver_dependencies(
        &mut self,
        version: Version,
        top_level: bool,
        deps: &[Dependency],
    ) {
        self.versions
            .entry(version)
            .or_insert_with(|| DepVersion::new(top_level))
            .add_dependencies(deps);
    }

    pub fn add_modify_ver_dependent(
        &mut self,
        version: Version,
        top_level: bool,
        dependent: Package,
    ) {
        self.versions
            .entry(version)
            .or_insert_with(|| DepVersion::new(top_level))
            .add_dependent(dependent);
    }
}

// *** Package ***

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Package {
    pub name: Name,
    pub version: Version,
}

impl Ord for Package {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name
            .cmp(&other.name)
            .then_with(|| self.version.cmp(&other.version))
    }
}

impl PartialOrd for Package {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Display for Package {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.name, self.version)
    }
}

// *** DepVersion ***

#[derive(Debug)]
pub struct DepVersion {
    dependencies: IndexSet<Package>,
    dependents: IndexSet<Package>,
    top_level: bool,
}

impl DepVersion {
    pub fn new(top_level: bool) -> Self {
        Self {
            dependencies: IndexSet::new(),
            dependents: IndexSet::new(),
            top_level,
        }
    }

    pub fn is_top_level(&self) -> bool {
        self.top_level || self.dependents.is_empty()
    }

    pub fn dependencies(&self) -> &IndexSet<Package> {
        &self.dependencies
    }

    pub fn dependents(&self) -> &IndexSet<Package> {
        &self.dependents
    }

    fn add_dependencies(&mut self, deps: &[cargo_lock::Dependency]) {
        self.dependencies = deps
            .iter()
            .map(|dep| Package {
                name: dep.name.clone(),
                version: dep.version.clone(),
            })
            .collect();
    }

    fn add_dependent(&mut self, dependent: Package) {
        self.dependents.insert(dependent);
    }
}
