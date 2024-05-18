use crate::{
    dep_tree::{Deps, Package},
    multi_ver_deps::MultiVerDep,
};

use cargo_lock::{Name, Version};
use indexmap::{IndexMap, IndexSet};

// *** MultiVerDeps ***

#[derive(Default)]
pub(crate) struct MultiVerDeps {
    deps: IndexMap<Name, IndexSet<Version>>,
}

impl MultiVerDeps {
    fn add(&mut self, name: Name, version: Version) {
        self.deps.entry(name).or_default().insert(version);
    }

    pub fn multi_ver_iter(&self) -> impl Iterator<Item = (&Name, &IndexSet<Version>)> {
        self.deps.iter().filter(|(_, versions)| versions.len() > 1)
    }

    pub fn has_all(&self, name: &Name, versions: &IndexSet<Version>) -> bool {
        match self.deps.get(name) {
            Some(dep_versions) => dep_versions.is_superset(versions),
            None => false,
        }
    }
}

// *** MultiVerParents ***

pub(crate) struct MultiVerParents {
    parents: IndexMap<Name, IndexMap<Version, MultiVerDeps>>,
}

impl MultiVerParents {
    pub fn build(
        deps: &Deps,
        multi_ver_deps: &IndexMap<Name, MultiVerDep>,
    ) -> Result<Self, String> {
        let mut multi_ver_parents = Self {
            parents: IndexMap::new(),
        };

        for (name, mv_dep) in multi_ver_deps {
            for version in mv_dep.versions() {
                let pkg = Package {
                    name: name.clone(),
                    version: version.clone(),
                };
                multi_ver_parents.build_multi_ver_parents(&pkg, deps)?;
            }
        }

        Ok(multi_ver_parents)
    }

    fn build_multi_ver_parents(&mut self, pkg: &Package, deps: &Deps) -> Result<(), String> {
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

            for dependent in ver.dependents() {
                next(pkg, dependent, deps, parents)?;
            }

            Ok(())
        }

        next(pkg, pkg, deps, self)
    }

    fn add(&mut self, parent_name: Name, parent_ver: Version, name: Name, ver: Version) {
        self.parents
            .entry(parent_name)
            .or_default()
            .entry(parent_ver)
            .or_default()
            .add(name, ver);
    }

    pub fn get_multi_ver_deps(&self, name: &Name, ver: &Version) -> Option<&MultiVerDeps> {
        self.parents.get(name)?.get(ver)
    }
}
