use std::collections::VecDeque;

use crate::blame::{MultiVerDepBlame, MultiVerDepBlameEntry};
use crate::dep_tree::Deps;
use crate::multi_ver_deps::MultiVerDeps;
use crate::multi_ver_parents::MultiVerDepParents;
use crate::{BlameMode, NO_DUP};

pub struct MultiVerDepResults {
    /// Top level packages that have multiple versions of dependencies
    top_level_blame: MultiVerDepBlame,

    /// Dependency packages that have multiple versions of dependencies
    dep_blame: MultiVerDepBlame,

    /// Dependencies that have multiple versions and their associated direct and top level dependents
    multi_ver_deps: MultiVerDeps,
}

impl MultiVerDepResults {
    pub fn build(
        deps: &Deps,
        parents: &MultiVerDepParents,
        multi_ver_deps: MultiVerDeps,
        blame_mode: Option<BlameMode>,
    ) -> Result<Self, String> {
        let mut top_level_blame = MultiVerDepBlame::default();
        let mut dep_blame = MultiVerDepBlame::default();

        if let Some(blame_mode) = blame_mode {
            let top_level_iter = deps.top_level_iter();
            let capacity = match blame_mode {
                // # of top level packages
                BlameMode::TopLevel => top_level_iter.len(),
                // # of packages in total
                BlameMode::All => deps.iter().len(),
            };
            let mut work_queue = VecDeque::with_capacity(capacity);
            work_queue.extend(top_level_iter.map(|pkg| (pkg, true)));

            while let Some((pkg, is_top_level)) = work_queue.pop_front() {
                // Only process if we haven't processed this before
                // NOTE: Because it is not always obvious if something is top level or not
                // we check both top level and dependency blame
                if !top_level_blame.contains(pkg) && !dep_blame.contains(pkg) {
                    let dep_ver = deps.get_version(pkg)?;
                    let deps = dep_ver.dependencies();
                    let blame = MultiVerDepBlameEntry::build(pkg, deps, parents);

                    if is_top_level {
                        top_level_blame.insert(pkg.clone(), blame);
                    } else {
                        dep_blame.insert(pkg.clone(), blame);
                    }

                    // Only keep recursing into tree if we want depenencies as well as top level
                    if blame_mode == BlameMode::All {
                        // Some of these MIGHT be top level, but we always process top level first
                        // so blame will already be assigned
                        work_queue.extend(deps.iter().map(|dep| (dep, false)));
                    }
                }
            }

            top_level_blame.sort();
            dep_blame.sort();
        }

        Ok(Self {
            top_level_blame,
            dep_blame,
            multi_ver_deps,
        })
    }

    pub fn return_error(&self, blame_mode: Option<BlameMode>) -> bool {
        match blame_mode {
            // Only top level having direct blame is an issue
            Some(BlameMode::TopLevel) => self.top_level_blame.has_direct_blame(),
            // Either top level or dependencies having direct blame is an issue
            Some(BlameMode::All) => {
                self.top_level_blame.has_direct_blame() || self.dep_blame.has_direct_blame()
            }
            // No blame mode we just care if we have any multi version dependencies
            _ => !self.multi_ver_deps.is_empty(),
        }
    }

    pub fn render<W: std::fmt::Write>(
        &self,
        w: &mut W,
        count: usize,
        blame_mode: Option<BlameMode>,
        blame_detail: bool,
    ) -> std::fmt::Result {
        if !self.multi_ver_deps.is_empty() {
            writeln!(w, "Duplicate Package(s):\n")?;
            writeln!(w, "{}", self.multi_ver_deps)?;

            if blame_mode.is_some() {
                writeln!(w, "Top Level Blame:\n")?;
                self.top_level_blame.render(w, blame_detail)?;
            }

            if let Some(BlameMode::All) = blame_mode {
                writeln!(w, "\nDependency Blame:\n")?;
                self.dep_blame.render(w, blame_detail)?;
            }

            writeln!(w, "\nSummary:\n")?;

            writeln!(
                w,
                "{} duplicate out of {} total package(s) ({} duplicate versions)",
                self.multi_ver_deps.dup_pkg_count(),
                count,
                self.multi_ver_deps.dup_ver_count(),
            )?;

            if blame_mode.is_some() {
                writeln!(
                    w,
                    "{} top level package(s) to blame ({} directly, {} indirectly, {} both)",
                    self.top_level_blame.count(),
                    self.top_level_blame.direct_count(),
                    self.top_level_blame.indirect_count(),
                    self.top_level_blame.both_count()
                )?;
            }

            if let Some(BlameMode::All) = blame_mode {
                writeln!(
                    w,
                    "{} dependency package(s) to blame ({} directly, {} indirectly, {} both)",
                    self.dep_blame.count(),
                    self.dep_blame.direct_count(),
                    self.dep_blame.indirect_count(),
                    self.dep_blame.both_count()
                )?;
            }
        } else {
            writeln!(w, "{NO_DUP}No duplicate dependencies found.{NO_DUP:#}")?;
        }

        Ok(())
    }
}
