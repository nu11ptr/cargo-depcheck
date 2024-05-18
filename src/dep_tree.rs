use std::hash::Hash;

use cargo_lock::{Dependency, Lockfile, Name, ResolveVersion, Version};
use indexmap::{map::Entry, IndexMap, IndexSet};

use crate::multi_ver_deps::{DupDepResults, MultiVerDep, MultiVerParents};

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

            match deps.entry(package.name.clone()) {
                // Already present by being a dependency of another package
                Entry::Occupied(mut entry) => {
                    let dep: &mut Dep = entry.get_mut();
                    dep.add_modify_ver_dependencies(
                        package.version.clone(),
                        top_level,
                        &package.dependencies,
                    );
                }
                // First time seeing this package
                Entry::Vacant(entry) => {
                    let mut dep = Dep::new(package.name.clone());
                    dep.add_modify_ver_dependencies(
                        package.version.clone(),
                        top_level,
                        &package.dependencies,
                    );
                    entry.insert(dep);
                }
            }

            // Add all dependents
            for dependency in package.dependencies {
                let dependent = Package {
                    name: package.name.clone(),
                    version: package.version.clone(),
                };

                let top_level = dependency.source.is_none();

                match deps.entry(dependency.name.clone()) {
                    Entry::Occupied(mut entry) => {
                        let dep: &mut Dep = entry.get_mut();
                        dep.add_modify_ver_dependent(dependency.version, top_level, dependent);
                    }
                    Entry::Vacant(entry) => {
                        // Assume not a top level package since we don't have that info right now
                        let mut dep = Dep::new(dependency.name.clone());
                        dep.add_modify_ver_dependent(dependency.version, top_level, dependent);
                        entry.insert(dep);
                    }
                }
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

        let mut multi_ver_parents = MultiVerParents::default();

        for (name, mv_dep) in &multi_ver_deps {
            for version in mv_dep.versions() {
                let pkg = Package {
                    name: name.clone(),
                    version: version.clone(),
                };
                self.build_multi_ver_parents(&pkg, &mut multi_ver_parents)?;
            }
        }

        DupDepResults::from_multi_ver_deps_parents(
            multi_ver_deps,
            &multi_ver_parents,
            show_deps,
            show_dups,
            verbose,
            self,
        )
    }

    fn build_multi_ver_parents(
        &self,
        pkg: &Package,
        parents: &mut MultiVerParents,
    ) -> Result<(), String> {
        fn next(
            pkg: &Package,
            curr_pkg: &Package,
            deps: &Deps,
            parents: &mut MultiVerParents,
        ) -> Result<(), String> {
            let ver = deps.get_version(curr_pkg)?;

            if pkg != curr_pkg {
                parents.add(
                    curr_pkg.name.clone(),
                    curr_pkg.version.clone(),
                    pkg.name.clone(),
                    pkg.version.clone(),
                );
            }

            for dependent in &ver.dependents {
                next(pkg, dependent, deps, parents)?;
            }

            Ok(())
        }

        next(pkg, pkg, self, parents)
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
        match self.versions.entry(version.clone()) {
            Entry::Occupied(mut entry) => {
                let version = entry.get_mut();
                version.add_dependencies(deps);
            }
            Entry::Vacant(entry) => {
                let mut version = DepVersion::new(version, top_level);
                version.add_dependencies(deps);
                entry.insert(version);
            }
        }
    }

    pub fn add_modify_ver_dependent(
        &mut self,
        version: Version,
        top_level: bool,
        dependent: Package,
    ) {
        match self.versions.entry(version.clone()) {
            Entry::Occupied(mut entry) => {
                let version = entry.get_mut();
                version.add_dependent(dependent);
            }
            Entry::Vacant(entry) => {
                let mut version = DepVersion::new(version, top_level);
                version.add_dependent(dependent);
                entry.insert(version);
            }
        }
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
    version: Version,
    dependencies: IndexSet<Package>,
    dependents: IndexSet<Package>,
    top_level: bool,
}

impl DepVersion {
    pub fn new(version: Version, top_level: bool) -> Self {
        Self {
            version,
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
