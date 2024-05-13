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
                    dep.top_level = package.source.is_none();
                    dep.add_modify_ver_dependencies(package.version.clone(), &package.dependencies);
                }
                // First time seeing this package
                Entry::Vacant(entry) => {
                    let mut dep = Dep::new(package.name.clone(), package.source.is_none());
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
                        // Assume not a top level package since we don't have that info right now
                        let mut dep = Dep::new(dependency.name.clone(), false);
                        dep.add_modify_ver_dependent(dependency.version, dependent);
                        entry.insert(dep);
                    }
                }
            }
        }

        let mut deps = Deps { deps };
        deps.sort();
        Ok(deps)
    }

    fn sort(&mut self) {
        self.deps.values_mut().for_each(|dep| dep.sort());
        self.deps.sort_unstable_keys();
    }

    fn find_dependents(
        &self,
        pkg: &Package,
    ) -> Result<IndexMap<Package, IndexSet<Package>>, Box<dyn std::error::Error>> {
        let mut dependents = IndexMap::new();

        fn next(
            deps: &IndexMap<Name, Dep>,
            curr_pkg: &Package,
            prev_pkg: &Package,
            direct_dep_pkg: Option<Package>,
            dependents: &mut IndexMap<Package, IndexSet<Package>>,
        ) -> Result<(), String> {
            let dep = deps.get(&curr_pkg.name).ok_or(format!(
                "Corrupted lock file: Dependency '{}' not found",
                curr_pkg.name
            ))?;

            let ver = dep.versions.get(&curr_pkg.version).ok_or(format!(
                "Corrupted lock file: Version '{}' of '{}' not found",
                curr_pkg.version, curr_pkg.name
            ))?;

            // If a local project or no dependents then we have found the top level
            if dep.top_level || ver.dependents.is_empty() {
                let direct_dep_pkg = direct_dep_pkg.unwrap_or_else(|| curr_pkg.clone());
                let top_level_set = dependents.entry(direct_dep_pkg.clone()).or_default();

                let direct_dep = deps.get(&direct_dep_pkg.name).ok_or(format!(
                    "Corrupted lock file: Dependency '{}' not found",
                    curr_pkg.name
                ))?;

                // If top level is part of our local project then it doesn't tell us much
                // so we use the previous package as the top level (unless it is top_level itself)
                let top_level_dep = if dep.top_level && !direct_dep.top_level {
                    prev_pkg.clone()
                } else {
                    curr_pkg.clone()
                };

                // Don't insert top level dependent if same as direct dependent
                if top_level_dep != direct_dep_pkg {
                    top_level_set.insert(top_level_dep);
                }

                return Ok(());
            // If it has dependents then we need to recurse higher up the tree
            } else {
                for dependent in &ver.dependents {
                    let direct_dep = direct_dep_pkg.clone().or_else(|| Some(dependent.clone()));
                    next(deps, dependent, curr_pkg, direct_dep, dependents)?;
                }
            }

            Ok::<_, String>(())
        }

        next(&self.deps, pkg, pkg, None, &mut dependents)?;
        dependents.values_mut().for_each(|set| set.sort_unstable());
        Ok(dependents)
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
    top_level: bool,
    versions: IndexMap<Version, DepVersion>,
}

impl Dep {
    pub fn new(name: Name, top_level: bool) -> Self {
        Self {
            name,
            top_level,
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
                    let dependents = deps.find_dependents(&pkg)?;
                    Ok::<_, Box<dyn std::error::Error>>((ver.version.clone(), dependents))
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

    fn sort(&mut self) {
        self.versions
            .values_mut()
            .for_each(|version| version.sort());
        self.versions.sort_unstable_keys();
    }
}

// *** Pkg ***

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct Package {
    name: Name,
    version: Version,
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

    fn sort(&mut self) {
        self.dependencies.sort_unstable();
        self.dependents.sort_unstable();
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
    versions: IndexMap<Version, IndexMap<Package, IndexSet<Package>>>,
}

impl std::fmt::Display for DuplicateDep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}:", self.name)?;

        for (version, direct_deps) in &self.versions {
            writeln!(f, "  {}:", version)?;

            for (direct, top_level_deps) in direct_deps {
                writeln!(f, "    {}", direct)?;

                for top_level in top_level_deps {
                    writeln!(f, "      {}", top_level)?;
                }
            }
        }

        Ok(())
    }
}
