use anstyle::{AnsiColor, Style};

pub(crate) const DIRECT: Style = AnsiColor::Red.on_default();
pub(crate) const INDIRECT: Style = AnsiColor::Yellow.on_default();
pub(crate) const NO_DUP: Style = AnsiColor::Green.on_default();

pub(crate) mod blame;
pub mod dep_tree;
pub(crate) mod multi_ver_deps;
pub(crate) mod multi_ver_parents;
pub mod results;
