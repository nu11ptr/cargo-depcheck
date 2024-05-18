use crate::{Package, DIRECT, INDIRECT, NO_DUP};

use cargo_lock::Name;
use indexmap::{IndexMap, IndexSet};

// *** ParentDepResponsibility ***

/// Tracks direct and indirect multi version depencency responsibility for a given package
pub(crate) struct ParentDepResponsibility {
    /// Name and version of the package that has multiple versions
    package: Package,

    /// Packages that have multiple versions this package is directly responsible for including
    direct: IndexSet<Name>,

    /// Packages that have multiple versions this package is indirectly responsible for (by including
    /// a package that itself has direct/indirect multi version responsibilities)
    indirect: IndexSet<Name>,

    verbose: bool,
}

impl std::fmt::Display for ParentDepResponsibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let style = if self.has_direct_responsibilities() {
            DIRECT
        } else if self.has_indirect_responsibilities() {
            INDIRECT
        } else {
            NO_DUP
        };

        writeln!(
            f,
            "{style}  {} (direct: {}, indirect: {}){style:#}",
            self.package,
            self.direct.len(),
            self.indirect.len()
        )?;

        if self.verbose {
            if self.has_direct_responsibilities() {
                writeln!(f, "{DIRECT}    Direct:")?;
                for name in &self.direct {
                    writeln!(f, "      {name}")?;
                }
                write!(f, "{DIRECT:#}")?;
            }

            if self.has_indirect_responsibilities() {
                writeln!(f, "{INDIRECT}    Indirect:")?;
                for name in &self.indirect {
                    writeln!(f, "      {name}")?;
                }
                write!(f, "{INDIRECT:#}")?;
            }
        }

        Ok(())
    }
}

impl ParentDepResponsibility {
    pub fn new(package: Package, verbose: bool) -> Self {
        Self {
            package,
            direct: IndexSet::new(),
            indirect: IndexSet::new(),
            verbose,
        }
    }

    pub fn add_direct(&mut self, name: Name) {
        self.direct.insert(name);
    }

    pub fn add_indirect(&mut self, name: Name) {
        self.indirect.insert(name);
    }

    pub fn has_direct_responsibilities(&self) -> bool {
        !self.direct.is_empty()
    }

    pub fn has_indirect_responsibilities(&self) -> bool {
        !self.indirect.is_empty()
    }

    pub fn has_responsibilities(&self) -> bool {
        self.has_direct_responsibilities() || self.has_indirect_responsibilities()
    }

    pub fn sort(&mut self) {
        self.direct.sort_unstable();
        self.indirect.sort_unstable();
    }
}

// *** ParentDepResponsibilities ***

#[derive(Default)]
pub(crate) struct ParentDepResponsibilities(IndexMap<Package, ParentDepResponsibility>);

impl ParentDepResponsibilities {
    pub fn insert(&mut self, package: Package, resp: ParentDepResponsibility) {
        self.0.insert(package, resp);
    }

    pub fn contains(&self, package: &Package) -> bool {
        self.0.contains_key(package)
    }

    pub fn has_direct_responsibilities(&self) -> bool {
        self.0
            .values()
            .any(|responsible| responsible.has_direct_responsibilities())
    }

    pub fn has_indirect_responsibilities(&self) -> bool {
        self.0
            .values()
            .any(|responsible| responsible.has_indirect_responsibilities())
    }

    pub fn has_responsibilities(&self) -> bool {
        self.0
            .values()
            .any(|responsible| responsible.has_responsibilities())
    }

    pub fn sort(&mut self) {
        self.0
            .values_mut()
            .for_each(|responsible| responsible.sort());
        self.0.sort_unstable_keys();
    }
}

impl std::fmt::Display for ParentDepResponsibilities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for responsible in self.0.values() {
            write!(f, "{responsible}")?;
        }

        Ok(())
    }
}
