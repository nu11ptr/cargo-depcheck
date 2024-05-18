use cargo_lock::{Dependency, Lockfile, Name, ResolveVersion, Version};
use indexmap::{IndexMap, IndexSet};

use crate::{
    multi_ver_deps::MultiVerDeps, multi_ver_parents::MultiVerParents, results::DupDepResults,
    Package,
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

            let dep: &mut Dep = deps.entry(package.name.clone()).or_default();
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

                let dep = deps.entry(dependency.name.clone()).or_default();
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

    pub fn iter(&self) -> impl Iterator<Item = (&Name, &Dep)> {
        self.deps.iter()
    }

    pub fn build_dup_dep_results(
        &self,
        show_deps: bool,
        show_dups: bool,
        verbose: bool,
    ) -> Result<DupDepResults, String> {
        let multi_ver_deps = MultiVerDeps::from_deps(self);
        let multi_ver_parents = MultiVerParents::from_deps(self, &multi_ver_deps)?;

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

#[derive(Debug, Default)]
pub struct Dep {
    versions: IndexMap<Version, DepVersion>,
}

impl Dep {
    pub fn has_multiple_versions(&self) -> bool {
        self.versions.len() > 1
    }

    pub fn versions(&self) -> IndexSet<Version> {
        self.versions.keys().cloned().collect()
    }

    fn add_modify_ver_dependencies(
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

    fn add_modify_ver_dependent(&mut self, version: Version, top_level: bool, dependent: Package) {
        self.versions
            .entry(version)
            .or_insert_with(|| DepVersion::new(top_level))
            .add_dependent(dependent);
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
