use super::*;

pub(super) fn declared_item_name(item: &syn::Item) -> Option<String> {
    let ident = match item {
        syn::Item::Const(i) => &i.ident,
        syn::Item::Enum(i) => &i.ident,
        syn::Item::Fn(i) => &i.sig.ident,
        syn::Item::Mod(i) => &i.ident,
        syn::Item::Static(i) => &i.ident,
        syn::Item::Struct(i) => &i.ident,
        syn::Item::Trait(i) => &i.ident,
        syn::Item::TraitAlias(i) => &i.ident,
        syn::Item::Type(i) => &i.ident,
        syn::Item::Union(i) => &i.ident,
        _ => return None,
    };
    Some(ident.to_string())
}

/// Check if a path starts with `crate`, `self`, `super`, or the crate's own name.
pub(super) fn reexport_message(name: &str, item_scope: &ModulePath) -> String {
    if *item_scope == ModulePath::root() {
        format!("`{name}` is only public within the crate, and cannot be re-exported outside")
    } else {
        format!("`{name}` is private, and cannot be re-exported")
    }
}

impl<'a> ValidationVisitor<'a> {
    pub(super) fn check_reexport_visibility(
        &mut self,
        item: &ItemInfo,
        links: &[Vis],
        name: &str,
        binds_value: bool,
        use_vis: &Vis,
        span: proc_macro2::Span,
    ) {
        // Links are listed nearest the item first, so the last one is nearest the re-export.
        let narrowing_scope = links
            .iter()
            .rev()
            .chain(std::iter::once(&item.vis))
            .find_map(|vis| match (vis, use_vis) {
                (Vis::Restricted(scope), Vis::Public) => Some(scope),
                (Vis::Restricted(scope), Vis::Restricted(use_scope))
                    if !use_scope.0.starts_with(&scope.0) =>
                {
                    Some(scope)
                }
                _ => None,
            });
        let Some(narrowing_scope) = narrowing_scope else {
            return;
        };
        let code = if binds_value { "E0364" } else { "E0365" };
        let message = reexport_message(name, narrowing_scope);
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

    /// Whether `name` is brought into the current module or its enclosing blocks by an item,
    /// a `use`, or a glob that may bring any name. `skip_glob` is the glob being checked.
    pub(super) fn name_in_scope(&self, name: &str, skip_glob: &[String]) -> bool {
        let symbols = self.symbols;
        let glob_may_bring = |prefix: &[String]| {
            if prefix == skip_glob {
                return false;
            }
            let target = symbols
                .resolve_use_path(&self.module_path, prefix, self.crate_name)
                .and_then(|abs| symbols.walk_path(&abs, &self.module_path, self.crate_name).module());
            match target {
                Some(module) => {
                    !matches!(
                        symbols.resolve_item(&module.path, name, &self.module_path, self.crate_name),
                        Resolved::NotFound
                    ) || self.module_may_hide(&module.path, name, &mut Vec::new())
                }
                // Outside the crate: it may bring any name, unless its own first
                // segment is unknown too (an unresolved glob brings nothing).
                None => prefix.first().is_some_and(|first| {
                    self.is_known_crate(first)
                        || !matches!(
                            symbols.resolve_item(
                                &self.module_path,
                                first,
                                &self.module_path,
                                self.crate_name
                            ),
                            Resolved::NotFound
                        )
                        || self
                            .local_scopes
                            .iter()
                            .any(|s| s.imports.iter().any(|i| i.name == *first))
                }),
            }
        };
        for scope in &self.local_scopes {
            if scope.imports.iter().any(|i| i.name == name)
                || scope.binds(name)
                || scope.globs.iter().any(|g| glob_may_bring(g))
            {
                return true;
            }
        }
        if !matches!(
            symbols.resolve_item(&self.module_path, name, &self.module_path, self.crate_name),
            Resolved::NotFound
        ) {
            return true;
        }
        self.module_uses()
            .any(|u| if u.is_glob { glob_may_bring(&u.path) } else { u.alias == name })
    }

    /// Paths of the globs in scope: those of the enclosing blocks, then the module's own.
    pub(super) fn scope_globs(&self) -> Vec<&[String]> {
        let block_globs = self.local_scopes.iter().flat_map(|s| s.globs.iter().map(Vec::as_slice));
        block_globs.chain(self.symbols.glob_paths(&self.module_path)).collect()
    }

    /// Resolve a name visible in the current module (own items, uses, re-exports, globs).
    pub(super) fn lookup_in_scope(&self, name: &str) -> Option<&ItemInfo> {
        if let LocalLookup::Item(item) = self.lookup_local(name) {
            return Some(item);
        }
        self.symbols
            .resolve_item(&self.module_path, name, &self.module_path, self.crate_name)
            .accessible()
    }

}

impl<'a, 'b> RefVisitor<'a, 'b> {
    /// The scope holds globs, all of them into modules of this crate that cannot hide `name`.
    pub(super) fn unknown_behind_local_globs(&self, name: &str) -> bool {
        let v = &*self.validator;
        let globs = v.scope_globs();
        if globs.is_empty() || v.symbols.has_macro_call(&v.module_path) {
            return false;
        }
        globs.into_iter().all(|prefix| {
            v.symbols
                .resolve_use_path(&v.module_path, prefix, v.crate_name)
                .and_then(|abs| v.symbols.walk_path(&abs, &v.module_path, v.crate_name).module())
                .is_some_and(|target| !v.module_may_hide(&target.path, name, &mut Vec::new()))
        })
    }

    /// Check if a name is in scope; if not, suggest where it lives in the crate.
    /// Generates a Fix (insert use statement) when there's a single unambiguous candidate,
    /// or uses smart import resolution when there are multiple candidates.
    pub(super) fn check_name_in_scope(&mut self, name: &str, span: proc_macro2::Span) {
        let in_scope = PRELUDE_NAMES.contains(&name)
            || !matches!(self.validator.lookup_local(name), LocalLookup::None)
            || self.validator.module_uses().any(|u| !u.is_glob && u.alias == name)
            || self.validator.lookup_in_scope(name).is_some()
            // An item-level macro invocation in this module may define the name.
            || self.validator.symbols.has_macro_call(&self.validator.module_path);

        if in_scope {
            return;
        }

        if let Some(private_in) = self.private_behind_glob(name) {
            self.validator.diagnostics.push(Diagnostic {
                severity: Severity::Error,
                file: self.validator.file_path.to_path_buf(),
                line: span.start().line,
                column: span.start().column + 1,
                message: format!("cannot find `{name}` in this scope"),
                error_code: Some("E0425".to_string()),
                hint: Some(format!("`{name}` is private in `{private_in}`, a glob does not import it")),
                fix: None,
            });
            return;
        }

        let candidates: Vec<(String, ItemKind)> = self.validator
            .find_anywhere(name)
            .iter()
            .map(|(path, item)| (path.display(), item.kind.clone()))
            .collect();

        if candidates.is_empty() {
            if self.unknown_behind_local_globs(name) {
                self.validator.diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    file: self.validator.file_path.to_path_buf(),
                    line: span.start().line,
                    column: span.start().column + 1,
                    message: format!("cannot find `{name}` in this scope"),
                    error_code: Some("E0425".to_string()),
                    hint: None,
                    fix: None,
                });
            }
            return; // Not in crate and no glob can hide it: could be external, stay quiet
        }

        let (fix, hint) = if candidates.len() == 1 {
            // Single candidate — high confidence auto-fix
            let insert_line = self.validator.find_use_insert_line();
            let use_path = &candidates[0].0;
            let fix = Fix::InsertLine {
                file: self.validator.file_path.to_path_buf(),
                line: insert_line,
                content: format!("use {use_path}::{name};\n"),
            };
            let hint = format!("add `use {use_path}::{name};`");
            (Some(fix), hint)
        } else {
            // Multiple candidates — report all, no auto-fix
            let locations: Vec<&str> = candidates.iter().map(|(p, _)| p.as_str()).collect();
            let hint = format!(
                "did you mean `{name}` from `{}`?",
                locations.join("` or `")
            );
            (None, hint)
        };

        self.validator.diagnostics.push(Diagnostic {
            severity: Severity::Suggestion,
            file: self.validator.file_path.to_path_buf(),
            line: span.start().line,
            column: span.start().column + 1,
            message: format!("cannot find `{name}` in this scope"),
            // rustc 1.101 reports E0425 for a missing type and a missing value alike.
            error_code: Some("E0425".to_string()),
            hint: Some(hint),
            fix,
        });
    }

}
