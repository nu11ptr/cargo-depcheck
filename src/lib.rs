use anstyle::{AnsiColor, Style};
use cargo_lock::{Name, Version};

pub(crate) const DIRECT: Style = AnsiColor::Red.on_default();
pub(crate) const INDIRECT: Style = AnsiColor::Yellow.on_default();
pub(crate) const NO_DUP: Style = AnsiColor::Green.on_default();

pub(crate) mod blame;
pub(crate) mod dep_tree;
pub(crate) mod multi_ver_deps;
pub(crate) mod multi_ver_parents;
pub(crate) mod results;

pub use dep_tree::*;
pub use multi_ver_deps::MultiVerDeps;
pub use multi_ver_parents::MultiVerParents;
pub use results::MultiVerDepResults;

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
