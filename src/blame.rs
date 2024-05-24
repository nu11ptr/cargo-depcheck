//! This module tracks blame both directly and indirectly of parents, their dependencies and the duplicate dependency dependencies.
//!
//! The structure tree looks like this: Parent -> Dup Dep Name -> Dup Dep Version -> Parent Dependency

use crate::{MultiVerDepParents, Package, DIRECT, INDIRECT, NO_DUP};

use anstyle::{AnsiColor, Style};
use cargo_lock::{Name, Version};
use indexmap::{IndexMap, IndexSet};

const TL_DEP: Style = AnsiColor::Blue.on_default();

// *** MultiVerDepBlameDep ***

/// The dependencies directly specified by the top level parent package
#[derive(Default)]
pub(crate) struct MultiVerDepBlameDep(IndexSet<Package>);

impl MultiVerDepBlameDep {
    pub fn insert(&mut self, package: Package) {
        self.0.insert(package);
    }

    pub fn sort(&mut self) {
        self.0.sort_unstable();
    }

    pub fn render<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        write!(w, "{TL_DEP}--> ")?;

        for (idx, pkg) in self.0.iter().enumerate() {
            write!(w, "{pkg}")?;
            if idx < self.0.len() - 1 {
                write!(w, ", ")?;
            }
        }

        writeln!(w, "{TL_DEP:#}")
    }
}

// *** MultiVerDepBlameVer ***

/// Tracks duplicate dependency version and it's mapping to the top level parent dependency
pub(crate) struct MultiVerDepBlameVer(IndexMap<Version, MultiVerDepBlameDep>);

impl MultiVerDepBlameVer {
    pub fn build(
        name: &Name,
        versions: &IndexSet<Version>,
        parent_deps: &IndexSet<Package>,
        parents: &MultiVerDepParents,
    ) -> Self {
        let mut ver_entries = Self(IndexMap::new());

        // Process all the dependencies of the parent package...
        for parent_dep in parent_deps {
            // ... but we only process dependencies that have our same multi version dependency
            if let Some(dep_mv_deps) = parents.get_multi_ver_deps(parent_dep) {
                // Only if dependency has some versions of it do we do anything at all
                if let Some(dep_versions) = dep_mv_deps.get(name) {
                    // If dependency versions aren't equal to parent's then there must be
                    // at least one other dependency that has a different version therefore
                    // we are directly to blame
                    if dep_versions != versions {
                        // Keep track of all versions of the dependency used by this package
                        for dep_ver in dep_versions {
                            ver_entries
                                .0
                                .entry(dep_ver.clone())
                                .or_default()
                                .insert(parent_dep.clone());
                        }
                    }
                }
            }
        }

        ver_entries.0.values_mut().for_each(|pkg| pkg.sort());
        ver_entries.0.sort_unstable_keys();
        ver_entries
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn render<W: std::fmt::Write>(&self, w: &mut W, name: &Name) -> std::fmt::Result {
        for (version, deps) in &self.0 {
            deps.render(w)?;
            writeln!(w, "      {name} {version}")?;
        }

        Ok(())
    }
}

// *** MultiVerDepBlameEntry ***

/// Tracks direct and indirect multi version depencency responsibility for a given package
pub(crate) struct MultiVerDepBlameEntry {
    /// Packages that have multiple versions this package is directly responsible for including
    direct: IndexMap<Name, MultiVerDepBlameVer>,

    /// Packages that have multiple versions this package is indirectly responsible for (by including
    /// a package that itself has direct/indirect multi version responsibilities)
    indirect: IndexSet<Name>,
}

impl MultiVerDepBlameEntry {
    pub fn build(
        parent_pkg: &Package,
        parent_deps: &IndexSet<Package>,
        parents: &MultiVerDepParents,
    ) -> Self {
        let mut entry = Self {
            direct: IndexMap::new(),
            indirect: IndexSet::new(),
        };

        // We only assign blame if we are a dependent of a multi version dependency
        if let Some(multi_ver_deps) = parents.get_multi_ver_deps(parent_pkg) {
            // Handle each package where we are a dependent of the dependency
            for (name, versions) in multi_ver_deps.multi_ver_iter() {
                // If true, we know we have multiple versions, so direct or indirect blame
                // will be assigned beyond this point
                if versions.len() > 1 {
                    let direct_blame_deps =
                        MultiVerDepBlameVer::build(name, versions, parent_deps, parents);

                    // If we have entries than we are to blame directly otherwise indirectly
                    if direct_blame_deps.is_empty() {
                        entry.indirect.insert(name.clone());
                    } else {
                        entry.direct.insert(name.clone(), direct_blame_deps);
                    }
                }
            }
        }

        entry.direct.sort_unstable_keys();
        entry.indirect.sort_unstable();
        entry
    }

    pub fn has_direct_blame(&self) -> bool {
        !self.direct.is_empty()
    }

    pub fn has_indirect_blame(&self) -> bool {
        !self.indirect.is_empty()
    }

    pub fn render<W: std::fmt::Write>(&self, w: &mut W, blame_detail: bool) -> std::fmt::Result {
        let style = if self.has_direct_blame() {
            DIRECT
        } else if self.has_indirect_blame() {
            INDIRECT
        } else {
            NO_DUP
        };

        writeln!(
            w,
            " (direct: {}, indirect: {}){style:#}",
            self.direct.len(),
            self.indirect.len()
        )?;

        if blame_detail && self.has_direct_blame() {
            writeln!(w, "  Direct:")?;
            for (name, versions) in &self.direct {
                versions.render(w, name)?;
            }
        }

        Ok(())
    }
}

// *** MultiVerDepBlame ***

/// Top level representing the package mapping to the duplication dependencies
#[derive(Default)]
pub(crate) struct MultiVerDepBlame(IndexMap<Package, MultiVerDepBlameEntry>);

impl MultiVerDepBlame {
    pub fn insert(&mut self, package: Package, resp: MultiVerDepBlameEntry) {
        self.0.insert(package, resp);
    }

    pub fn sort(&mut self) {
        self.0.sort_unstable_keys();
    }

    pub fn contains(&self, package: &Package) -> bool {
        self.0.contains_key(package)
    }

    pub fn has_direct_blame(&self) -> bool {
        self.0.values().any(|entry| entry.has_direct_blame())
    }

    pub fn direct_count(&self) -> usize {
        self.0
            .values()
            .filter(|entry| entry.has_direct_blame() && !entry.has_indirect_blame())
            .count()
    }

    pub fn indirect_count(&self) -> usize {
        self.0
            .values()
            .filter(|entry| entry.has_indirect_blame() && !entry.has_direct_blame())
            .count()
    }

    pub fn both_count(&self) -> usize {
        self.0
            .values()
            .filter(|entry| entry.has_direct_blame() && entry.has_indirect_blame())
            .count()
    }

    pub fn count(&self) -> usize {
        self.0
            .values()
            .filter(|entry| entry.has_direct_blame() || entry.has_indirect_blame())
            .count()
    }

    pub fn render<W: std::fmt::Write>(&self, w: &mut W, blame_detail: bool) -> std::fmt::Result {
        for (package, resp) in self.0.iter() {
            let style = if resp.has_direct_blame() {
                DIRECT
            } else if resp.has_indirect_blame() {
                INDIRECT
            } else {
                NO_DUP
            };

            write!(w, "{style}{package}")?;
            resp.render(w, blame_detail)?;
        }

        Ok(())
    }
}
