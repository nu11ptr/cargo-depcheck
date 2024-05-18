use crate::blame::{ParentDepResponsibilities, ParentDepResponsibility};
use crate::dep_tree::{Deps, Package};
use crate::NO_DUP;

use cargo_lock::{Name, Version};
use indexmap::{IndexMap, IndexSet};

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

// ### Multi Version Dependency Tracking ###

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

/// Represents a dependency that has multiple versions. It can track 3 levels of hierarchy:
/// the direct dependent, the top level's dependencies, and the top level dependents. It intentionally
/// skips the levels between the direct dependent and the top level dependents for brevity.
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

// ### Results ###

// *** DupDepResults ***

pub struct DupDepResults {
    /// Top level packages that have multiple versions of dependencies
    top_level_responsible: ParentDepResponsibilities,

    /// Dependency packages that have multiple versions of dependencies
    dep_responsible: ParentDepResponsibilities,

    /// Dependencies that have multiple versions and their associated direct and top level dependents
    multi_ver_deps: IndexMap<Name, MultiVerDep>,

    verbose: bool,
    show_deps: bool,
    show_dups: bool,
}

impl std::fmt::Display for DupDepResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.has_dup_deps() {
            if self.top_level_responsible.has_responsibilities() {
                writeln!(f, "Top Level Packages with Multi Version Dependencies:\n")?;
                writeln!(f, "{}\n", self.top_level_responsible)?;
            }

            if self.show_deps && self.dep_responsible.has_responsibilities() {
                writeln!(f, "Dependencies with Multi Version Dependencies:\n")?;
                writeln!(f, "{}\n", self.dep_responsible)?;
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
        let mut top_level_responsible = ParentDepResponsibilities::default();
        let mut dep_responsible = ParentDepResponsibilities::default();

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
                    &mut dep_responsible,
                    parents,
                    deps,
                    verbose,
                )?;
            }
        }

        top_level_responsible.sort();
        dep_responsible.sort();
        // TODO: Create new type and sort at every level?
        multi_ver_deps.sort_unstable_keys();

        Ok(Self {
            top_level_responsible,
            dep_responsible,
            multi_ver_deps,
            show_deps,
            show_dups,
            verbose,
        })
    }

    fn process_multi_ver_dep(
        pkg: &Package,
        dt_parents: &mut DependencyParents,
        top_level_responsible: &mut ParentDepResponsibilities,
        dep_responsible: &mut ParentDepResponsibilities,
        parents: &MultiVerParents,
        deps: &Deps,
        verbose: bool,
    ) -> Result<(), String> {
        fn next(
            curr_pkg: &Package,
            prev_pkg: &Package,
            direct_tl_dep_pkg: Option<(&Package, Option<(&Package, Option<&Package>)>)>,
            dt_parents: &mut DependencyParents,
            top_level_responsible: &mut ParentDepResponsibilities,
            dep_responsible: &mut ParentDepResponsibilities,
            parents: &MultiVerParents,
            deps: &Deps,
            verbose: bool,
        ) -> Result<(), String> {
            let dep_ver = deps.get_version(curr_pkg)?;
            let top_level = dep_ver.is_top_level();

            let processed = if top_level {
                top_level_responsible.contains(curr_pkg)
            } else {
                dep_responsible.contains(curr_pkg)
            };

            // This package may have been processed already by another multi version dependency
            if !processed {
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

                    if top_level {
                        top_level_responsible.insert(curr_pkg.clone(), parent_multi_ver_deps);
                    } else {
                        dep_responsible.insert(curr_pkg.clone(), parent_multi_ver_deps);
                    }
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
                    dep_responsible,
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
            dep_responsible,
            parents,
            deps,
            verbose,
        )
    }

    pub fn has_dup_deps(&self) -> bool {
        !self.multi_ver_deps.is_empty()
    }
}
