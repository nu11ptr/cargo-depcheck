# cargo-depcheck

A Cargo plugin that checks for duplicate dependencies. It can simply report them, but optionally can find out which packages are to blame. Lastly, it can display inverse trees of these dependencies. All these tools make it easy to find and fix duplicate dependencies.

## Status

Alpha - only eyeball tests have been done. Automated tests forthcoming.

## Operation

TODO

## Terminology

* `Dependency` = something required to build a package. It can be directly specified in the `Cargo.toml` file or further downstream (a dependency of a dependency).

* `Dependent` = a package that requires the given package to build itself. It is the inverse of a dependency.

* `Version` = a single version of a given package.

* `Multi Version Dependency` = a package included in the project either directly or indirectly (via another dependency) where multiple versions of the package are present.

* `Lockfile` = the `Cargo.lock` file present in source tree

* `Package` = A standard Rust package. In our context this will typically be a library crate (except at top level).

* `Node` = A package in the dependency tree

* `Top Level` = a package directly specified in the source tree. It will show up in the `Cargo.lock` file without a "source" entry. If it does have dependents, they will also be local in the source tree.

* `Top Level Dependency` = a dependency of a top level package specified in a `Cargo.toml` file.

* `Direct Dependent` = The package that includes a version of the multi version dependency directly in it's `Cargo.toml` file.

* `Direct Blame` = A package that includes two or more dependencies where each of these dependencies uses a different version of the same package.

* `Indirect Blame` = A package that has multiple versions of a dependency, however, all versions of the package came from a single dependency.

## Algorithm

First, we loop over all the dependencies in the lock file and build a basic tree (not actually a tree structure, stored as maps/sets). For each node, we track both it's dependencies AND it's dependents so we can walk the tree up and down. Each node is a dependency that tracks all the versions present in the lockfile as a single node.

Second, once we have our dependencies all built, it is easy to iterate over them and only keep the ones with multiple versions. We call this our `MultiVerDeps` structure. The tool in its simplest form stops here and reports them.

Third, if we are looking to find out what is to blame for these multiple dependencies, we take the `MultiVerDeps` we found in the second step and use them to walk up the tree from the bottom to the top by following the dependents. At each level, we take the version of the multi version dependency we started with and store it against that dependent. We call this our `MultiVerParents` structure, and it is used as a helper structure in the next two sections.

NOTE: The next two sections happen together, if applicable, based on configuration. They will be mentioned independently for clarity.

Fourth, we once again loop over our `MultiVerDeps` structure, but also will reference `MultiVerParents` as we go. Once again we will walk up the tree of our dependents and check at each package verson whether we have processed this one already or not. If we haven't, we loop over each dependency it has in `MultiVerParents` comparing it to all of it's dependencies entries in `MultiVerParents`. If any single dependency has all the same versions of the dependency we do, then we add the package's name as an "indirect" blame for the dependency (read: one of its dependencies was responsible for the multiple version dependency). Otherwise, it is considered directly to blame for having multiple copies of the dependency (since no single dependency has all its versions).

Lastly, as we walk up the tree, we calculate direct, top level package dependencies and top level packages for each multi version dependency. We skip any intermediate nodes for brevity since they are generally less interesting. The first time we process a dependent we store the dependent as the direct dependent for the dependency. Once we reach the top level (defined as a package with no "source" key in the lockfile), we store the top level and previous node as as the top level dependency. In case our tree simply isn't that deep, we perform some tests and stop at the top level without repeating any nodes (for example, if a top level package directly has a dependency then it will only have a direct entry and top level dependency and top level will not be set).
