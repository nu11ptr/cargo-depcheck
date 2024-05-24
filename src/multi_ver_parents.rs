use crate::{dep_tree::Deps, Package};

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

    pub fn get(&self, name: &Name) -> Option<&IndexSet<Version>> {
        self.deps.get(name)
    }

    pub fn multi_ver_iter(&self) -> impl Iterator<Item = (&Name, &IndexSet<Version>)> {
        self.deps.iter().filter(|(_, versions)| versions.len() > 1)
    }
}

// *** MultiVerParents ***

#[derive(Default)]
pub struct MultiVerDepParents {
    parents: IndexMap<Package, MultiVerDeps>,
}

impl MultiVerDepParents {
    pub fn build(
        deps: &Deps,
        multi_ver_deps: &crate::multi_ver_deps::MultiVerDeps,
    ) -> Result<Self, String> {
        let mut multi_ver_parents = Self {
            parents: IndexMap::new(),
        };

        for (name, mv_dep) in multi_ver_deps.iter() {
            for version in mv_dep.iter() {
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
            parents: &mut MultiVerDepParents,
        ) -> Result<(), String> {
            let ver = deps.get_version(curr_pkg)?;

            if pkg != curr_pkg {
                parents.add(curr_pkg.clone(), pkg.name.clone(), pkg.version.clone());
            }

            for dependent in ver.dependents() {
                next(pkg, dependent, deps, parents)?;
            }

            Ok(())
        }

        next(pkg, pkg, deps, self)
    }

    fn add(&mut self, parent: Package, name: Name, ver: Version) {
        self.parents.entry(parent).or_default().add(name, ver);
    }

    pub(crate) fn get_multi_ver_deps(&self, parent: &Package) -> Option<&MultiVerDeps> {
        self.parents.get(parent)
    }
}
