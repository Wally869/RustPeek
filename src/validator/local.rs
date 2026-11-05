use super::*;

/// The privacy rule shared by `use` paths and expression paths: a path is private when the
/// module it goes through, or the item it names, is not visible to the accessor.
pub(super) fn is_private_step(module_accessible: bool, found: Resolved) -> bool {
    !module_accessible || matches!(found, Resolved::Inaccessible(_))
}

pub(super) fn collect_local_imports(tree: &syn::UseTree, prefix: &mut Vec<String>, scope: &mut LocalScope) {
    match tree {
        syn::UseTree::Path(p) => {
            prefix.push(p.ident.to_string());
            collect_local_imports(&p.tree, prefix, scope);
            prefix.pop();
        }
        syn::UseTree::Name(n) => {
            let ident = n.ident.to_string();
            if ident == "self" {
                if let Some(last) = prefix.last() {
                    scope.imports.push(LocalImport { name: last.clone(), path: prefix.clone() });
                }
            } else {
                let mut path = prefix.clone();
                path.push(ident.clone());
                scope.imports.push(LocalImport { name: ident, path });
            }
        }
        syn::UseTree::Rename(r) => {
            let alias = r.rename.to_string();
            if alias != "_" {
                let mut path = prefix.clone();
                path.push(r.ident.to_string());
                scope.imports.push(LocalImport { name: alias, path });
            }
        }
        syn::UseTree::Glob(_) => scope.globs.push(prefix.clone()),
        syn::UseTree::Group(g) => {
            for tree in &g.items {
                collect_local_imports(tree, prefix, scope);
            }
        }
    }
}

pub(super) fn is_crate_path(path: &[String], crate_name: Option<&str>) -> bool {
    path.first().is_some_and(|s| {
        s == "crate" || s == "self" || s == "super"
            || crate_name.is_some_and(|name| name == s)
    })
}

/// Get the last segment name from a path.
pub(super) fn path_last_segment(path: &syn::Path) -> Option<String> {
    path.segments.last().map(|s| s.ident.to_string())
}

impl<'a> ValidationVisitor<'a> {
    pub(super) fn glob_outcome(&self, prefix: &[String]) -> GlobOutcome {
        let symbols = self.symbols;
        let Some(abs) = symbols.resolve_use_path(&self.module_path, prefix, self.crate_name)
        else {
            return GlobOutcome::Unresolved;
        };
        let walk = symbols.walk_path(&abs, &self.module_path, self.crate_name);
        if walk.stop.is_none() && symbols.module_exists(&walk.reached) {
            return if walk.accessible { GlobOutcome::Resolved } else { GlobOutcome::Private };
        }
        // The prefix names an item (or a module without a file): its parent modules must
        // all have been walked, so only the last segment may be where the walk stopped.
        let Some(last) = abs.len().checked_sub(1) else {
            return GlobOutcome::Unresolved;
        };
        let last_is_item = match walk.stop {
            None => true,
            Some(PathStop::NotModule { index }) if index == last => true,
            Some(PathStop::Missing { index }) if index == last => false,
            Some(PathStop::External { .. }) => return GlobOutcome::Resolved,
            Some(_) => return GlobOutcome::Unresolved,
        };
        if walk.first_private.is_some_and(|index| index < last) {
            return GlobOutcome::Private;
        }
        if last_is_item && walk.first_private.is_none() {
            GlobOutcome::Resolved
        } else {
            GlobOutcome::Unresolved
        }
    }

    /// A path is local when it starts with `crate`/`self`/`super`/the crate name,
    /// or with a child module of the current module.
    pub(super) fn is_local_path(&self, path: &[String]) -> bool {
        is_crate_path(path, self.crate_name)
            || (!path.is_empty()
                && self
                    .symbols
                    .resolve_use_path(&self.module_path, path, self.crate_name)
                    .is_some())
    }

    /// The first segment of a non-local `use` prefix (the path without a glob or the
    /// imported name) names no crate and nothing in scope. Off when the dependency
    /// list is unknown.
    pub(super) fn is_unknown_external_crate(&self, prefix: &[String]) -> bool {
        if self.symbols.external_crates.is_none() {
            return false;
        }
        let Some(first) = prefix.first() else {
            return false;
        };
        // A prelude name is a valid path root too (`use Option::{self as X, None}`).
        !(self.is_known_crate(first)
            || PRELUDE_NAMES.contains(&first.as_str())
            || self.symbols.has_macro_call(&self.module_path)
            || self.name_in_scope(first, prefix))
    }

    /// Reports a non-local `use` whose first segment is unknown. rustc: E0432 when the
    /// unknown crate is the last segment of the prefix, E0433 when more segments follow.
    pub(super) fn report_unknown_crate(&mut self, prefix: &[String], span: proc_macro2::Span) {
        if !self.is_unknown_external_crate(prefix) {
            return;
        }
        self.push_unknown_root(prefix, span);
    }

    /// The diagnostic for a path root that is not a module or crate (see `report_unknown_crate`).
    pub(super) fn push_unknown_root(&mut self, prefix: &[String], span: proc_macro2::Span) {
        let (message, code) = if prefix.len() > 1 {
            (format!("cannot find module or crate `{}`", prefix[0]), "E0433")
        } else {
            (format!("unresolved import `{}`", prefix[0]), "E0432")
        };
        self.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            file: self.file_path.to_path_buf(),
            line: span.start().line,
            column: span.start().column + 1,
            message,
            error_code: Some(code.to_string()),
            hint: None,
            fix: None,
        });
    }

    /// A crate name the current crate can use without `extern crate`.
    pub(super) fn is_known_crate(&self, name: &str) -> bool {
        self.symbols.is_known_crate(name)
    }

    /// Resolve a path written in the current module through its own segments: a single
    /// name is looked up in scope; a longer path must name a crate module then an item in
    /// it. `None` for a path through a type (`Type::f`), an external path or a missing item.
    pub(super) fn lookup_path(&self, path: &[String], namespace: Namespace) -> Option<&ItemInfo> {
        match path {
            [name] => self.lookup_in_scope(name, namespace),
            _ => self.lookup_crate_path(path, namespace),
        }
    }

    /// The accessible item a `use`-style path names, when every segment before the last
    /// is a crate module.
    pub(super) fn lookup_crate_path(
        &self,
        path: &[String],
        namespace: Namespace,
    ) -> Option<&ItemInfo> {
        let symbols = self.symbols;
        let abs = symbols.resolve_use_path(&self.module_path, path, self.crate_name)?;
        let (item_name, module_part) = abs.split_last()?;
        let module = symbols.walk_path(module_part, &self.module_path, self.crate_name).module()?;
        symbols
            .resolve_item(&module.path, item_name, namespace, &self.module_path, self.crate_name)
            .accessible()
    }

    /// Names brought in by `use` items of the enclosing blocks, innermost first.
    pub(super) fn lookup_local(&self, name: &str, namespace: Namespace) -> LocalLookup<'_> {
        let mut imported = false;
        let symbols = self.symbols;
        for scope in self.local_scopes.iter().rev() {
            imported |= scope.binds(name);
            for import in scope.imports.iter().filter(|i| i.name == name) {
                imported = true;
                if let Some(item) = self.lookup_crate_path(&import.path, namespace) {
                    return LocalLookup::Item(item);
                }
            }
            for prefix in &scope.globs {
                let target = symbols
                    .resolve_use_path(&self.module_path, prefix, self.crate_name)
                    .and_then(|abs| {
                        symbols.walk_path(&abs, &self.module_path, self.crate_name).module()
                    });
                match target {
                    Some(module) => {
                        if let Some(item) = symbols
                            .resolve_item(
                                &module.path,
                                name,
                                namespace,
                                &self.module_path,
                                self.crate_name,
                            )
                            .accessible()
                        {
                            return LocalLookup::Item(item);
                        }
                    }
                    // External or non-module target: any name may come from it
                    None => imported = true,
                }
            }
        }
        if imported { LocalLookup::Imported } else { LocalLookup::None }
    }

}

impl<'a, 'b> RefVisitor<'a, 'b> {
    pub(super) fn report_private(&mut self, path: &syn::Path, segment_index: usize, name: &str) {
        let span = path.segments[segment_index].ident.span();
        self.validator.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            file: self.validator.file_path.to_path_buf(),
            line: span.start().line,
            column: span.start().column + 1,
            message: format!("`{name}` is private"),
            error_code: Some("E0603".to_string()),
            hint: None,
            fix: None,
        });
    }

    pub(super) fn private_behind_glob(&self, name: &str) -> Option<String> {
        let v = &*self.validator;
        let symbols = v.symbols;
        v.scope_globs().into_iter().find_map(|prefix| {
            let abs = symbols.resolve_use_path(&v.module_path, prefix, v.crate_name)?;
            let target = symbols.walk_path(&abs, &v.module_path, v.crate_name).module()?;
            match symbols.resolve_item(&target.path, name, Namespace::Any, &v.module_path, v.crate_name) {
                Resolved::Inaccessible(_) => Some(target.path.display()),
                _ => None,
            }
        })
    }

}
