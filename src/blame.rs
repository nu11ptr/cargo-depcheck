use crate::{BlamePkgMode, Package, DIRECT, INDIRECT, NO_DUP};

use cargo_lock::Name;
use indexmap::{IndexMap, IndexSet};

// *** ParentBlameEntry ***

/// Tracks direct and indirect multi version depencency responsibility for a given package
#[derive(Default)]
pub(crate) struct ParentBlameEntry {
    /// Packages that have multiple versions this package is directly responsible for including
    direct: IndexSet<Name>,

    /// Packages that have multiple versions this package is indirectly responsible for (by including
    /// a package that itself has direct/indirect multi version responsibilities)
    indirect: IndexSet<Name>,
}

impl ParentBlameEntry {
    pub fn add_direct(&mut self, name: Name) {
        self.direct.insert(name);
    }

    pub fn add_indirect(&mut self, name: Name) {
        self.indirect.insert(name);
    }

    pub fn has_direct_blame(&self) -> bool {
        !self.direct.is_empty()
    }

    pub fn has_indirect_blame(&self) -> bool {
        !self.indirect.is_empty()
    }

    pub fn has_blame(&self) -> bool {
        self.has_direct_blame() || self.has_indirect_blame()
    }

    pub fn has_direct_blame_for(&self, name: &Name) -> bool {
        self.direct.contains(name)
    }

    pub fn has_indirect_blame_for(&self, name: &Name) -> bool {
        self.indirect.contains(name)
    }

    pub fn sort(&mut self) {
        self.direct.sort_unstable();
        self.indirect.sort_unstable();
    }

    pub fn render<W: std::fmt::Write>(
        &self,
        w: &mut W,
        blame_packages: Option<BlamePkgMode>,
    ) -> std::fmt::Result {
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

        if blame_packages.is_some() {
            if self.has_direct_blame() {
                writeln!(w, "{DIRECT}    Direct:")?;
                for name in &self.direct {
                    writeln!(w, "      {name}")?;
                }
                write!(w, "{DIRECT:#}")?;
            }

            if self.has_indirect_blame() && matches!(blame_packages, Some(BlamePkgMode::All)) {
                writeln!(w, "{INDIRECT}    Indirect:")?;
                for name in &self.indirect {
                    writeln!(w, "      {name}")?;
                }
                write!(w, "{INDIRECT:#}")?;
            }
        }

        Ok(())
    }
}

// *** ParentMultiVerBlame ***

#[derive(Default)]
pub(crate) struct ParentMultiVerBlame(IndexMap<Package, ParentBlameEntry>);

impl ParentMultiVerBlame {
    pub fn insert(&mut self, package: Package, resp: ParentBlameEntry) {
        self.0.insert(package, resp);
    }

    pub fn contains(&self, package: &Package) -> bool {
        self.0.contains_key(package)
    }

    pub fn has_direct_blame_for(&self, parent: &Package, dep: &Name) -> bool {
        match self.0.get(parent) {
            Some(entry) => entry.has_direct_blame_for(dep),
            None => false,
        }
    }

    pub fn has_indirect_blame_for(&self, parent: &Package, dep: &Name) -> bool {
        match self.0.get(parent) {
            Some(entry) => entry.has_indirect_blame_for(dep),
            None => false,
        }
    }

    pub fn has_blame(&self) -> bool {
        self.0.values().any(|entry| entry.has_blame())
    }

    pub fn sort(&mut self) {
        self.0.values_mut().for_each(|entry| entry.sort());
        self.0.sort_unstable_keys();
    }

    pub fn render<W: std::fmt::Write>(
        &self,
        w: &mut W,
        blame_packages: Option<BlamePkgMode>,
    ) -> std::fmt::Result {
        for (package, resp) in self.0.iter() {
            let style = if resp.has_direct_blame() {
                DIRECT
            } else if resp.has_indirect_blame() {
                INDIRECT
            } else {
                NO_DUP
            };

            write!(w, "{style}  {package}")?;
            resp.render(w, blame_packages)?;
        }

        Ok(())
    }
}
