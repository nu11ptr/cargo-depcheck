use anstyle::{AnsiColor, Style};
use cargo_lock::{Name, Version};
use indexmap::{map::Entry, IndexMap, IndexSet};

use crate::dep_tree::{Deps, Package};

const DIRECT: Style = AnsiColor::Red.on_default();
const INDIRECT: Style = AnsiColor::Yellow.on_default();
const NO_DUP: Style = AnsiColor::Green.on_default();

// *** MultiVerDeps ***

#[derive(Default)]
pub(crate) struct MultiVerDeps {
    deps: IndexMap<Name, IndexSet<Version>>,
}

impl MultiVerDeps {
    pub fn add(&mut self, name: Name, version: Version) {
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

#[derive(Default)]
pub(crate) struct MultiVerParents {
    parents: IndexMap<Name, IndexMap<Version, MultiVerDeps>>,
}

impl MultiVerParents {
    pub fn add(&mut self, parent_name: Name, parent_ver: Version, name: Name, ver: Version) {
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

// *** TopLevelParents ***

#[derive(Default)]
struct TopLevelPackages(IndexSet<Package>);

impl TopLevelPackages {
    fn add(&mut self, top_level: Package) {
        self.0.insert(top_level);
    }
}

impl std::fmt::Display for TopLevelPackages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for pkg in &self.0 {
            writeln!(f, "          {pkg}")?;
        }

        Ok(())
    }
}

// *** TopLevelDependencies ***

#[derive(Default)]
struct TopLevelDependencies(IndexMap<Package, TopLevelPackages>);

impl TopLevelDependencies {
    fn add(&mut self, top_level_dep: Package) -> &mut TopLevelPackages {
        self.0.entry(top_level_dep).or_default()
    }
}

impl std::fmt::Display for TopLevelDependencies {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (direct, top_level) in self.0.iter() {
            writeln!(f, "        {direct}")?;
            writeln!(f, "{top_level}")?;
        }

        Ok(())
    }
}

// *** DependencyParents ***

#[derive(Default)]
struct DependencyParents(IndexMap<Package, TopLevelDependencies>);

impl DependencyParents {
    fn add(&mut self, direct: Package) -> &mut TopLevelDependencies {
        self.0.entry(direct).or_default()
    }
}

impl std::fmt::Display for DependencyParents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (direct, top_level_deps) in self.0.iter() {
            writeln!(f, "      {direct}")?;
            write!(f, "{top_level_deps}")?;
        }

        Ok(())
    }
}

// *** MultiVerDep ***

pub(crate) struct MultiVerDep {
    name: Name,
    versions: IndexMap<Version, DependencyParents>,
}

impl MultiVerDep {
    pub fn new(name: Name, versions: IndexSet<Version>) -> Self {
        Self {
            name,
            versions: versions
                .into_iter()
                .map(|ver| (ver, DependencyParents::default()))
                .collect(),
        }
    }

    pub fn versions(&self) -> impl Iterator<Item = &Version> {
        self.versions.keys()
    }

    // fn add_parents(&mut self, version: Version, direct_and_top_level: DependencyParents) {
    //     self.versions.insert(version, direct_and_top_level);
    // }
}

impl std::fmt::Display for MultiVerDep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}:", self.name)?;

        for (version, direct_and_top_level) in self.versions.iter() {
            writeln!(f, "    {version}:")?;
            write!(f, "{direct_and_top_level}")?;
        }

        Ok(())
    }
}

// *** ParentDepResponsibility ***

/// Tracks direct and indirect multi version depencency responsibility for a given package
struct ParentDepResponsibility {
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

// *** DupDepResponsibilities ***

struct ParentDepResponsibilities(IndexMap<Package, ParentDepResponsibility>);

impl std::fmt::Display for ParentDepResponsibilities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for responsible in self.0.values() {
            write!(f, "{responsible}")?;
        }

        Ok(())
    }
}

impl ParentDepResponsibilities {
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

// *** DupDepResults ***

pub struct DupDepResults {
    /// Top level packages that have multiple versions of dependencies
    top_level: ParentDepResponsibilities,

    /// Dependency packages that have multiple versions of dependencies
    deps: ParentDepResponsibilities,

    // Dependencies that have multiple versions and their associated direct and top level dependents
    multi_ver_deps: IndexMap<Name, MultiVerDep>,

    verbose: bool,
    show_deps: bool,
    show_dups: bool,
}

impl std::fmt::Display for DupDepResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.has_dup_deps() {
            if self.top_level.has_responsibilities() {
                writeln!(f, "Top Level Packages with Multi Version Dependencies:\n")?;
                writeln!(f, "{}\n", self.top_level)?;
            }

            if self.show_deps && self.deps.has_responsibilities() {
                writeln!(f, "Dependencies with Multi Version Dependencies:\n")?;
                writeln!(f, "{}\n", self.deps)?;
            }

            if self.show_dups {
                writeln!(f, "Duplicate Package(s):")?;

                for multi_ver_dep in self.multi_ver_deps.values() {
                    writeln!(f, "  {multi_ver_dep}")?;
                }
            }
        } else {
            writeln!(f, "{NO_DUP}No duplicate dependencies found.{NO_DUP:#}")?;
        }

        Ok(())
    }
}

impl DupDepResults {
    pub(crate) fn from_multi_ver_deps_parents(
        mut multi_ver_deps: IndexMap<Name, MultiVerDep>,
        parents: &MultiVerParents,
        show_deps: bool,
        show_dups: bool,
        verbose: bool,
        deps: &Deps,
    ) -> Result<Self, String> {
        let mut top_level_responsible = IndexMap::new();
        let mut direct_responsible = IndexMap::new();

        for (name, mv_dep) in &mut multi_ver_deps {
            for (version, dt_parents) in mv_dep.versions.iter_mut() {
                let pkg = Package {
                    name: name.clone(),
                    version: version.clone(),
                };

                Self::process_multi_ver_dep(
                    &pkg,
                    dt_parents,
                    &mut top_level_responsible,
                    &mut direct_responsible,
                    parents,
                    deps,
                    verbose,
                )?;
            }
        }

        let mut top_level = ParentDepResponsibilities(top_level_responsible);
        let mut deps = ParentDepResponsibilities(direct_responsible);

        top_level.sort();
        deps.sort();
        // TODO: Create new type and sort at every level?
        multi_ver_deps.sort_unstable_keys();

        Ok(Self {
            top_level,
            deps,
            multi_ver_deps,
            show_deps,
            show_dups,
            verbose,
        })
    }

    fn process_multi_ver_dep(
        pkg: &Package,
        dt_parents: &mut DependencyParents,
        top_level_responsible: &mut IndexMap<Package, ParentDepResponsibility>,
        direct_responsible: &mut IndexMap<Package, ParentDepResponsibility>,
        parents: &MultiVerParents,
        deps: &Deps,
        verbose: bool,
    ) -> Result<(), String> {
        fn next(
            curr_pkg: &Package,
            prev_pkg: &Package,
            direct_tl_dep_pkg: Option<(&Package, Option<(&Package, Option<&Package>)>)>,
            dt_parents: &mut DependencyParents,
            top_level_responsible: &mut IndexMap<Package, ParentDepResponsibility>,
            direct_responsible: &mut IndexMap<Package, ParentDepResponsibility>,
            parents: &MultiVerParents,
            deps: &Deps,
            verbose: bool,
        ) -> Result<(), String> {
            let dep_ver = deps.get_version(curr_pkg)?;
            let top_level = dep_ver.is_top_level();

            let entry = if top_level {
                top_level_responsible.entry(curr_pkg.clone())
            } else {
                direct_responsible.entry(curr_pkg.clone())
            };

            // This package may have been processed already by another multi version dependency
            if let Entry::Vacant(entry) = entry {
                // Multi version deps if bottom rung may not themselves have this structure
                if let Some(multi_ver_deps) =
                    parents.get_multi_ver_deps(&curr_pkg.name, &curr_pkg.version)
                {
                    let mut parent_multi_ver_deps =
                        ParentDepResponsibility::new(curr_pkg.clone(), verbose);

                    // We only iterate over the dep if this immediate parent has multiple versions downstream
                    for (name, versions) in multi_ver_deps.multi_ver_iter() {
                        // Find out if any of our dependencies have all the versions
                        let has_all = dep_ver
                            .dependencies()
                            .iter()
                            .filter_map(|dep| parents.get_multi_ver_deps(&dep.name, &dep.version))
                            .any(|deps| deps.has_all(name, versions));

                        // If any single depedency has all the versions then we are only indirectly responsible else directly responsible
                        if has_all {
                            parent_multi_ver_deps.add_indirect(name.clone());
                        } else {
                            parent_multi_ver_deps.add_direct(name.clone());
                        }
                    }

                    entry.insert(parent_multi_ver_deps);
                }
            }

            if top_level {
                let (direct_tl_dep_pkg, store) = match direct_tl_dep_pkg {
                    // Most typical case. curr_pkg != prev_pkg. direct_pkg may or may not equal prev_pkg
                    Some((direct_pkg, None)) => {
                        // if true, our hierarchy is more than 2 levels deep
                        let pkg_deps = if direct_pkg != curr_pkg {
                            // if true, our hierarchy is more than 3 levels deep
                            if direct_pkg != prev_pkg {
                                Some((direct_pkg, Some((prev_pkg, Some(curr_pkg)))))
                            } else {
                                Some((direct_pkg, Some((curr_pkg, None))))
                            }
                        } else {
                            Some((curr_pkg, None))
                        };

                        (pkg_deps, true)
                    }

                    // This occurs when we have a top level package that also has top level dependents
                    // We are already done at this point
                    pkg_deps @ Some((_, Some(_))) => (pkg_deps, false),

                    // This only happens if a single level with no dependents (curr_pkg == prev_pkg == dependent_pkg)
                    None => (Some((curr_pkg, None)), true),
                };

                // If storing, store one level at a time based on how many levels we built
                if store {
                    if let Some((direct, rest)) = direct_tl_dep_pkg {
                        let top_level_deps = dt_parents.add(direct.clone());

                        if let Some((top_level_dep, rest)) = rest {
                            let top_level = top_level_deps.add(top_level_dep.clone());

                            if let Some(top_level_pkg) = rest {
                                top_level.add(top_level_pkg.clone());
                            }
                        }
                    }
                }
            }

            // Keep processing up stream until we reach the top level
            for dependent_pkg in dep_ver.dependents() {
                // If direct dep isn't yet set then it is the dependent we are about to process
                let direct_tl_dep_pkg = direct_tl_dep_pkg.or(Some((dependent_pkg, None)));

                next(
                    dependent_pkg,
                    curr_pkg,
                    direct_tl_dep_pkg,
                    dt_parents,
                    top_level_responsible,
                    direct_responsible,
                    parents,
                    deps,
                    verbose,
                )?;
            }

            Ok(())
        }

        next(
            pkg,
            pkg,
            None,
            dt_parents,
            top_level_responsible,
            direct_responsible,
            parents,
            deps,
            verbose,
        )
    }

    pub fn has_dup_deps(&self) -> bool {
        !self.multi_ver_deps.is_empty()
    }
}
