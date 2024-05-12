use std::hash::Hash;

use cargo_lock::{Dependency, Lockfile, Name, ResolveVersion, Version};
use indexmap::{map::Entry, IndexMap, IndexSet};

// *** Deps ***

#[derive(Debug)]
pub struct Deps {
    deps: IndexMap<Name, Dep>,
}

impl Deps {
    pub fn from_lock_file(lock_file: Lockfile) -> Result<Self, Box<dyn std::error::Error>> {
        let mut deps = IndexMap::with_capacity(lock_file.packages.len());

        // I can't find any examples of non-v3 lock files, so I'm not sure if this is necessary
        if lock_file.version != ResolveVersion::V3 {
            return Err("Only v3 lock files are supported".into());
        }

        for package in lock_file.packages {
            match deps.entry(package.name.clone()) {
                // Already present by being a dependency of another package
                Entry::Occupied(mut entry) => {
                    let dep: &mut Dep = entry.get_mut();
                    dep.add_modify_ver_dependencies(package.version.clone(), &package.dependencies);
                }
                // First time seeing this package
                Entry::Vacant(entry) => {
                    let mut dep = Dep::new(package.name.clone());
                    dep.add_modify_ver_dependencies(package.version.clone(), &package.dependencies);
                    entry.insert(dep);
                }
            }

            // Add all dependents
            for dependency in package.dependencies {
                match deps.entry(dependency.name.clone()) {
                    Entry::Occupied(mut entry) => {
                        let dep: &mut Dep = entry.get_mut();
                        dep.add_modify_ver_dependent(
                            dependency.version,
                            package.name.clone(),
                            package.version.clone(),
                        );
                    }
                    Entry::Vacant(entry) => {
                        let mut dep = Dep::new(dependency.name.clone());
                        dep.add_modify_ver_dependent(
                            dependency.version,
                            package.name.clone(),
                            package.version.clone(),
                        );
                        entry.insert(dep);
                    }
                }
            }
        }

        Ok(Self { deps })
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

    pub fn add_modify_ver_dependencies(&mut self, version: Version, deps: &[Dependency]) {
        match self.versions.entry(version.clone()) {
            Entry::Occupied(mut entry) => {
                let version = entry.get_mut();
                version.add_dependencies(deps);
            }
            Entry::Vacant(entry) => {
                let mut version = DepVersion::new(version);
                version.add_dependencies(deps);
                entry.insert(version);
            }
        }
    }

    pub fn add_modify_ver_dependent(&mut self, version: Version, dep_name: Name, dep_ver: Version) {
        match self.versions.entry(version.clone()) {
            Entry::Occupied(mut entry) => {
                let version = entry.get_mut();
                version.add_dependent(dep_name, dep_ver);
            }
            Entry::Vacant(entry) => {
                let mut version = DepVersion::new(version);
                version.add_dependent(dep_name, dep_ver);
                entry.insert(version);
            }
        }
    }
}

// *** DepVersion ***

#[derive(Debug)]
struct DepVersion {
    version: Version,
    dependencies: IndexSet<(Name, Version)>,
    dependents: IndexSet<(Name, Version)>,
}

impl DepVersion {
    pub fn new(version: Version) -> Self {
        Self {
            version,
            dependencies: IndexSet::new(),
            dependents: IndexSet::new(),
        }
    }

    pub fn add_dependencies(&mut self, deps: &[cargo_lock::Dependency]) {
        self.dependencies = deps
            .iter()
            .map(|dep| (dep.name.clone(), dep.version.clone()))
            .collect();
    }

    fn add_dependent(&mut self, name: Name, ver: Version) {
        self.dependents.insert((name, ver));
    }
}

impl PartialEq for DepVersion {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version
    }
}

impl Hash for DepVersion {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.version.hash(state);
    }
}
