use crate::blame::{ParentBlameEntry, ParentMultiVerBlame};
use crate::dep_tree::Deps;
use crate::multi_ver_deps::{DirectDependents, MultiVerDeps};
use crate::multi_ver_parents::MultiVerParents;
use crate::{BlameMode, BlamePkgMode, NO_DUP};
use crate::{DependentMode, Package};

pub struct MultiVerDepResults {
    /// Top level packages that have multiple versions of dependencies
    top_level_blame: ParentMultiVerBlame,

    /// Dependency packages that have multiple versions of dependencies
    dep_blame: ParentMultiVerBlame,

    /// Dependencies that have multiple versions and their associated direct and top level dependents
    multi_ver_deps: MultiVerDeps,
}

impl MultiVerDepResults {
    pub fn render<W: std::fmt::Write>(
        &self,
        w: &mut W,
        dep_mode: Option<DependentMode>,
        blame_mode: Option<BlameMode>,
        blame_packages: Option<BlamePkgMode>,
    ) -> std::fmt::Result {
        if self.has_multi_ver_deps() {
            if matches!(blame_mode, Some(BlameMode::TopLevel | BlameMode::All))
                && self.top_level_blame.has_blame()
            {
                writeln!(w, "Top Level Packages with Multi Version Dependencies:\n")?;
                self.top_level_blame.render(w, blame_packages)?;
                writeln!(w, "\n")?;
            }

            if matches!(blame_mode, Some(BlameMode::All)) && self.dep_blame.has_blame() {
                writeln!(w, "Dependencies with Multi Version Dependencies:\n")?;
                self.dep_blame.render(w, blame_packages)?;
                writeln!(w, "\n")?;
            }

            writeln!(w, "Duplicate Package(s):\n")?;
            self.multi_ver_deps
                .render(w, dep_mode, &self.top_level_blame, &self.dep_blame)?;
        } else {
            writeln!(w, "{NO_DUP}No duplicate dependencies found.{NO_DUP:#}")?;
        }

        Ok(())
    }

    pub fn build(
        deps: &Deps,
        parents: &MultiVerParents,
        mut multi_ver_deps: MultiVerDeps,
        dep_mode: Option<DependentMode>,
        blame_mode: Option<BlameMode>,
    ) -> Result<Self, String> {
        let mut top_level_blame = ParentMultiVerBlame::default();
        let mut dep_blame = ParentMultiVerBlame::default();

        if dep_mode.is_some() || blame_mode.is_some() {
            for (name, mv_dep) in multi_ver_deps.iter_mut() {
                for (version, dt_parents) in mv_dep.versions_mut() {
                    let pkg = Package {
                        name: name.clone(),
                        version: version.clone(),
                    };

                    Self::process_multi_ver_dep(
                        &pkg,
                        dt_parents,
                        &mut top_level_blame,
                        &mut dep_blame,
                        parents,
                        deps,
                        dep_mode,
                        blame_mode,
                    )?;
                }
            }

            top_level_blame.sort();
            dep_blame.sort();
        }

        // This will exist and need sorting regardless of modes
        multi_ver_deps.sort();

        Ok(Self {
            top_level_blame,
            dep_blame,
            multi_ver_deps,
        })
    }

    fn process_multi_ver_dep(
        pkg: &Package,
        dt_parents: &mut DirectDependents,
        top_level_blame: &mut ParentMultiVerBlame,
        dep_blame: &mut ParentMultiVerBlame,
        parents: &MultiVerParents,
        deps: &Deps,
        dep_mode: Option<DependentMode>,
        blame_mode: Option<BlameMode>,
    ) -> Result<(), String> {
        fn next(
            curr_pkg: &Package,
            prev_pkg: &Package,
            direct_tl_dep_pkg: Option<(&Package, Option<(&Package, Option<&Package>)>)>,
            deps: &Deps,
            parents: &MultiVerParents,
            dt_parents: &mut DirectDependents,
            top_level_blame: &mut ParentMultiVerBlame,
            dep_blame: &mut ParentMultiVerBlame,
            dep_mode: Option<DependentMode>,
            blame_mode: Option<BlameMode>,
        ) -> Result<(), String> {
            let dep_ver = deps.get_version(curr_pkg)?;
            let top_level = dep_ver.is_top_level();

            if blame_mode.is_some() {
                let processed = if top_level {
                    top_level_blame.contains(curr_pkg)
                } else {
                    dep_blame.contains(curr_pkg)
                };

                // This package may have been processed already by another multi version dependency
                if !processed {
                    // Multi version deps if bottom rung may not themselves have this structure
                    if let Some(multi_ver_deps) =
                        parents.get_multi_ver_deps(&curr_pkg.name, &curr_pkg.version)
                    {
                        let mut parent_multi_ver_deps = ParentBlameEntry::default();

                        // We only iterate over the dep if this immediate parent has multiple versions downstream
                        for (name, versions) in multi_ver_deps.multi_ver_iter() {
                            // Find out if any of our dependencies have all the versions
                            let has_all = dep_ver
                                .dependencies()
                                .iter()
                                .filter_map(|dep| {
                                    parents.get_multi_ver_deps(&dep.name, &dep.version)
                                })
                                .any(|deps| deps.has_all(name, versions));

                            // If any single depedency has all the versions then we are only indirectly responsible else directly responsible
                            if has_all {
                                parent_multi_ver_deps.add_indirect(name.clone());
                            } else {
                                parent_multi_ver_deps.add_direct(name.clone());
                            }
                        }

                        if top_level {
                            top_level_blame.insert(curr_pkg.clone(), parent_multi_ver_deps);
                        } else {
                            dep_blame.insert(curr_pkg.clone(), parent_multi_ver_deps);
                        }
                    }
                }
            }

            if top_level && dep_mode.is_some() {
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
                    deps,
                    parents,
                    dt_parents,
                    top_level_blame,
                    dep_blame,
                    dep_mode,
                    blame_mode,
                )?;
            }

            Ok(())
        }

        next(
            pkg,
            pkg,
            None,
            deps,
            parents,
            dt_parents,
            top_level_blame,
            dep_blame,
            dep_mode,
            blame_mode,
        )
    }

    pub fn has_multi_ver_deps(&self) -> bool {
        !self.multi_ver_deps.is_empty()
    }
}
