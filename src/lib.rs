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
                let dependent = Package {
                    name: package.name.clone(),
                    version: package.version.clone(),
                };

                match deps.entry(dependency.name.clone()) {
                    Entry::Occupied(mut entry) => {
                        let dep: &mut Dep = entry.get_mut();
                        dep.add_modify_ver_dependent(dependency.version, dependent);
                    }
                    Entry::Vacant(entry) => {
                        let mut dep = Dep::new(dependency.name.clone());
                        dep.add_modify_ver_dependent(dependency.version, dependent);
                        entry.insert(dep);
                    }
                }
            }
        }

        Ok(Self { deps })
    }

    fn find_top_level_dependents(
        &self,
        pkg: &Package,
    ) -> Result<IndexSet<Package>, Box<dyn std::error::Error>> {
        let mut top_level_deps = IndexSet::new();

        fn next(
            deps: &IndexMap<Name, Dep>,
            curr_pkg: &Package,
            top_level_deps: &mut IndexSet<Package>,
        ) -> Result<(), String> {
            let dep = deps.get(&curr_pkg.name).ok_or(format!(
                "Corrupted lock file: Dependency '{}' not found",
                curr_pkg.name
            ))?;

            let ver = dep.versions.get(&curr_pkg.version).ok_or(format!(
                "Corrupted lock file: Version '{}' of '{}' not found",
                curr_pkg.version, curr_pkg.name
            ))?;

            // If no dependents then we have found the top level
            if ver.dependents.is_empty() {
                top_level_deps.insert(curr_pkg.clone());
                return Ok(());
            // If it has dependents then we need to recurse higher up the tree
            } else {
                for dependent in &ver.dependents {
                    next(deps, dependent, top_level_deps)?;
                }
            }

            Ok::<_, String>(())
        }

        next(&self.deps, pkg, &mut top_level_deps)?;
        Ok(top_level_deps)
    }

    pub fn duplicate_versions(&self) -> Result<Vec<DuplicateDep>, Box<dyn std::error::Error>> {
        let dup_versions = self
            .deps
            .values()
            .map(|dep| dep.duplicate_versions(self))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(dup_versions.into_iter().flatten().collect())
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

    pub fn add_modify_ver_dependent(&mut self, version: Version, dependent: Package) {
        match self.versions.entry(version.clone()) {
            Entry::Occupied(mut entry) => {
                let version = entry.get_mut();
                version.add_dependent(dependent);
            }
            Entry::Vacant(entry) => {
                let mut version = DepVersion::new(version);
                version.add_dependent(dependent);
                entry.insert(version);
            }
        }
    }

    pub fn duplicate_versions(
        &self,
        deps: &Deps,
    ) -> Result<Option<DuplicateDep>, Box<dyn std::error::Error>> {
        if self.versions.len() > 1 {
            let versions = self
                .versions
                .values()
                .map(|ver| {
                    let pkg = Package {
                        name: self.name.clone(),
                        version: ver.version.clone(),
                    };
                    let top_level_dependents = deps.find_top_level_dependents(&pkg)?;
                    Ok::<_, Box<dyn std::error::Error>>((ver.version.clone(), top_level_dependents))
                })
                .collect::<Result<IndexMap<_, _>, _>>()?;

            Ok(Some(DuplicateDep {
                name: self.name.clone(),
                versions,
            }))
        } else {
            Ok(None)
        }
    }
}

// *** Pkg ***

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct Package {
    name: Name,
    version: Version,
}

impl std::fmt::Display for Package {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.name, self.version)
    }
}

// *** DepVersion ***

#[derive(Debug)]
struct DepVersion {
    version: Version,
    dependencies: IndexSet<Package>,
    dependents: IndexSet<Package>,
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

// *** DuplicateDep ***

#[derive(Debug)]
pub struct DuplicateDep {
    name: Name,
    versions: IndexMap<Version, IndexSet<Package>>,
}

impl std::fmt::Display for DuplicateDep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}:", self.name)?;

        for (version, dependents) in &self.versions {
            writeln!(f, "  {}:", version)?;

            for dependent in dependents {
                writeln!(f, "    {}", dependent)?;
            }
        }

        Ok(())
    }
}
