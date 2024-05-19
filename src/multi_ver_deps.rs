use crate::{
    blame::ParentMultiVerBlame, dep_tree::Deps, DependentMode, Package, DIRECT, INDIRECT, NO_DUP,
};

use anstyle::Style;
use cargo_lock::{Name, Version};
use indexmap::{IndexMap, IndexSet};

fn get_style(
    parent: &Package,
    dep: &Name,
    tl_blame: &ParentMultiVerBlame,
    dep_blame: &ParentMultiVerBlame,
) -> Style {
    if tl_blame.has_direct_blame_for(parent, dep) || dep_blame.has_direct_blame_for(parent, dep) {
        DIRECT
    } else if tl_blame.has_indirect_blame_for(parent, dep)
        || dep_blame.has_indirect_blame_for(parent, dep)
    {
        INDIRECT
    } else {
        NO_DUP
    }
}

// *** TopLevelPackages ***

#[derive(Default)]
pub(crate) struct TopLevelPackages(IndexSet<Package>);

impl TopLevelPackages {
    pub fn add(&mut self, top_level: Package) {
        self.0.insert(top_level);
    }

    pub fn render<W: std::fmt::Write>(
        &self,
        w: &mut W,
        dep_mode: Option<DependentMode>,
        name: &Name,
        tl_blame: &ParentMultiVerBlame,
        dep_blame: &ParentMultiVerBlame,
    ) -> std::fmt::Result {
        if let Some(DependentMode::All) = dep_mode {
            for pkg in &self.0 {
                let style = get_style(pkg, name, tl_blame, dep_blame);
                writeln!(w, "{style}          {pkg}{style:#}")?;
            }
        }

        Ok(())
    }
}

// *** TopLevelDependencies ***

#[derive(Default)]
pub(crate) struct TopLevelDependencies(IndexMap<Package, TopLevelPackages>);

impl TopLevelDependencies {
    pub fn add(&mut self, top_level_dep: Package) -> &mut TopLevelPackages {
        self.0.entry(top_level_dep).or_default()
    }

    pub fn render<W: std::fmt::Write>(
        &self,
        w: &mut W,
        dep_mode: Option<DependentMode>,
        name: &Name,
        tl_blame: &ParentMultiVerBlame,
        dep_blame: &ParentMultiVerBlame,
    ) -> std::fmt::Result {
        if matches!(
            dep_mode,
            Some(DependentMode::Dependency | DependentMode::All)
        ) {
            for (dep, top_level) in self.0.iter() {
                let style = get_style(dep, name, tl_blame, dep_blame);
                writeln!(w, "{style}        {dep}{style:#}")?;
                top_level.render(w, dep_mode, name, tl_blame, dep_blame)?;
            }
        }

        Ok(())
    }
}

// *** DirectDependents ***

#[derive(Default)]
pub(crate) struct DirectDependents(IndexMap<Package, TopLevelDependencies>);

impl DirectDependents {
    pub fn add(&mut self, direct: Package) -> &mut TopLevelDependencies {
        self.0.entry(direct).or_default()
    }

    pub fn render<W: std::fmt::Write>(
        &self,
        w: &mut W,
        dep_mode: Option<DependentMode>,
        name: &Name,
        tl_blame: &ParentMultiVerBlame,
        dep_blame: &ParentMultiVerBlame,
    ) -> std::fmt::Result {
        // Doesn't matter which mode since this is the first one
        if dep_mode.is_some() {
            for (direct, top_level_deps) in self.0.iter() {
                let style = get_style(direct, name, tl_blame, dep_blame);
                writeln!(w, "{style}      {direct}{style:#}")?;
                top_level_deps.render(w, dep_mode, name, tl_blame, dep_blame)?;
            }
        }

        Ok(())
    }
}

// *** MultiVerDep ***

/// Represents a dependency that has multiple versions. It can track 3 levels of hierarchy:
/// the direct dependent, the top level's dependencies, and the top level dependents. It intentionally
/// skips the levels between the direct dependent and the top level dependents for brevity.
pub(crate) struct MultiVerDep(IndexMap<Version, DirectDependents>);

impl MultiVerDep {
    pub fn new(versions: IndexSet<Version>) -> Self {
        Self(
            versions
                .into_iter()
                .map(|ver| (ver, DirectDependents::default()))
                .collect(),
        )
    }

    pub fn version_keys(&self) -> impl Iterator<Item = &Version> {
        self.0.keys()
    }

    pub fn versions_mut(&mut self) -> impl Iterator<Item = (&Version, &mut DirectDependents)> {
        self.0.iter_mut()
    }

    pub fn render<W: std::fmt::Write>(
        &self,
        w: &mut W,
        dep_mode: Option<DependentMode>,
        name: &Name,
        tl_blame: &ParentMultiVerBlame,
        dep_blame: &ParentMultiVerBlame,
    ) -> std::fmt::Result {
        if dep_mode.is_some() {
            for (version, direct) in self.0.iter() {
                writeln!(w, "    {version}:")?;
                direct.render(w, dep_mode, name, tl_blame, dep_blame)?;
            }
        } else {
            let versions = self
                .0
                .keys()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            w.write_str(&versions)?;
        }

        Ok(())
    }
}

// *** MultiVerDeps ***

pub struct MultiVerDeps(IndexMap<Name, MultiVerDep>);

impl MultiVerDeps {
    pub fn from_deps(deps: &Deps) -> Self {
        let multi_ver_deps: IndexMap<_, _> = deps
            .iter()
            .filter_map(|(name, dep)| {
                if dep.has_multiple_versions() {
                    Some((name.clone(), MultiVerDep::new(dep.versions())))
                } else {
                    None
                }
            })
            .collect();

        Self(multi_ver_deps)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&Name, &MultiVerDep)> {
        self.0.iter()
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = (&Name, &mut MultiVerDep)> {
        self.0.iter_mut()
    }

    pub fn sort(&mut self) {
        self.0.sort_unstable_keys();
    }

    pub(crate) fn render<W: std::fmt::Write>(
        &self,
        w: &mut W,
        dep_mode: Option<DependentMode>,
        tl_blame: &ParentMultiVerBlame,
        dep_blame: &ParentMultiVerBlame,
    ) -> std::fmt::Result {
        for (name, multi_ver_dep) in &self.0 {
            if dep_mode.is_some() {
                writeln!(w, "  {name}:")?;
                multi_ver_dep.render(w, dep_mode, name, tl_blame, dep_blame)?;
            } else {
                write!(w, "  {name} (")?;
                multi_ver_dep.render(w, dep_mode, name, tl_blame, dep_blame)?;
                writeln!(w, ")")?;
            }
        }

        Ok(())
    }
}
