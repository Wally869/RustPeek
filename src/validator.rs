use std::collections::HashSet;
use std::path::Path;

use syn::visit::Visit;

use crate::discovery;
use crate::types::*;

/// Validate references in a file's AST against the symbol table.
pub fn validate_file(
    ast: &syn::File,
    file_path: &Path,
    module_path: &ModulePath,
    symbols: &SymbolTable,
    src_dir: &Path,
    crate_name: Option<&str>,
    external_crates: Option<&HashSet<String>>,
) -> Vec<Diagnostic> {
    let mut macro_modules = HashSet::new();
    collect_macro_modules(&ast.items, module_path, &mut macro_modules);
    let mut visitor = ValidationVisitor {
        external_crates,
        macro_modules,
        diagnostics: Vec::new(),
        file_path,
        module_path: module_path.clone(),
        symbols,
        src_dir,
        source_lines: None,
        crate_name,
        local_scopes: Vec::new(),
    };

    visitor.validate_mod_declarations(ast);
    visitor.validate_use_statements(ast);
    visitor.validate_references(ast);

    let mut vis_check = VisPathCheck {
        diagnostics: &mut visitor.diagnostics,
        file_path,
        module_path: module_path.clone(),
    };
    vis_check.visit_file(ast);

    visitor.diagnostics
}

struct VisPathCheck<'a> {
    diagnostics: &'a mut Vec<Diagnostic>,
    file_path: &'a Path,
    module_path: ModulePath,
}

impl<'ast> Visit<'ast> for VisPathCheck<'_> {
    fn visit_item_mod(&mut self, m: &'ast syn::ItemMod) {
        self.visit_visibility(&m.vis);
        if let Some((_, inner)) = &m.content {
            let child = self.module_path.child(&m.ident.to_string());
            let outer = std::mem::replace(&mut self.module_path, child);
            for item in inner {
                self.visit_item(item);
            }
            self.module_path = outer;
        }
    }

    fn visit_vis_restricted(&mut self, r: &'ast syn::VisRestricted) {
        if resolve_vis_path(&r.path, &self.module_path).is_some() {
            return;
        }
        let path_text = r
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("::");
        let span = r.path.segments[0].ident.span();
        self.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            file: self.file_path.to_path_buf(),
            line: span.start().line,
            column: span.start().column + 1,
            message: format!(
                "failed to resolve `{path_text}`: there are too many leading `super` keywords"
            ),
            error_code: Some("E0433".to_string()),
            hint: None,
            fix: None,
        });
    }
}

#[derive(PartialEq, Eq)]
enum GlobOutcome {
    Resolved,
    Private,
    Unresolved,
}

struct ValidationVisitor<'a> {
    diagnostics: Vec<Diagnostic>,
    file_path: &'a Path,
    /// The file's module, or the inline module while inside its body.
    module_path: ModulePath,
    symbols: &'a SymbolTable,
    src_dir: &'a Path,
    /// Lazily loaded source lines for fix generation
    source_lines: Option<Vec<String>>,
    /// The crate's own name (from Cargo.toml), so `use <name>::...` is treated as `use crate::...`
    crate_name: Option<&'a str>,
    local_scopes: Vec<LocalScope>,
    /// None when the crate's dependencies are unknown.
    external_crates: Option<&'a HashSet<String>>,
    /// Modules with an item-level macro invocation, which may define any item.
    macro_modules: HashSet<ModulePath>,
}

fn collect_macro_modules(items: &[syn::Item], module: &ModulePath, out: &mut HashSet<ModulePath>) {
    for item in items {
        match item {
            syn::Item::Macro(_) => {
                out.insert(module.clone());
            }
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    collect_macro_modules(inner, &module.child(&m.ident.to_string()), out);
                }
            }
            _ => {}
        }
    }
}

impl<'a> ValidationVisitor<'a> {
    /// Get source lines, loading lazily.
    fn source_lines(&mut self) -> &[String] {
        if self.source_lines.is_none() {
            let content = std::fs::read_to_string(self.file_path).unwrap_or_default();
            self.source_lines = Some(content.lines().map(String::from).collect());
        }
        self.source_lines.as_deref().unwrap()
    }

    /// Find the best line to insert a `use` statement in the current file.
    /// Returns the line number to insert BEFORE (1-indexed).
    fn find_use_insert_line(&mut self) -> usize {
        let lines = self.source_lines().to_vec();
        let mut last_use_line = 0;
        let mut last_mod_line = 0;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("use ") {
                last_use_line = i + 1; // 1-indexed
            }
            if trimmed.starts_with("mod ") || trimmed.starts_with("pub mod ") {
                last_mod_line = i + 1;
            }
        }

        if last_use_line > 0 {
            last_use_line + 1 // Insert after last use
        } else if last_mod_line > 0 {
            last_mod_line + 1 // Insert after last mod
        } else {
            1 // Top of file
        }
    }

    /// Check that `mod foo;` declarations have corresponding files.
    fn validate_mod_declarations(&mut self, ast: &syn::File) {
        for item in &ast.items {
            if let syn::Item::Mod(m) = item {
                if m.content.is_some() {
                    continue;
                }

                let mod_name = m.ident.to_string();
                let resolved =
                    discovery::resolve_mod_file(self.src_dir, &self.module_path, &mod_name);

                if resolved.is_none() {
                    let span = m.ident.span();
                    self.diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        file: self.file_path.to_path_buf(),
                        line: span.start().line,
                        column: span.start().column + 1,
                        message: format!("file not found for module `{mod_name}`"),
                        error_code: Some("E0583".to_string()),
                        hint: Some(format!(
                            "expected `{mod_name}.rs` or `{mod_name}/mod.rs`"
                        )),
                        fix: None,
                    });
                }
            }
        }
    }

    /// Validate `use crate::...` statements resolve to actual items.
    fn validate_use_statements(&mut self, ast: &syn::File) {
        self.validate_use_items(&ast.items);
    }

    fn validate_use_items(&mut self, items: &[syn::Item]) {
        for item in items {
            match item {
                syn::Item::Use(u) => self.validate_use_tree(&u.tree, &mut Vec::new(), u),
                syn::Item::Mod(m) => {
                    if let Some((_, inner)) = &m.content {
                        let child = self.module_path.child(&m.ident.to_string());
                        let outer = std::mem::replace(&mut self.module_path, child);
                        self.validate_use_items(inner);
                        self.module_path = outer;
                    }
                }
                _ => {}
            }
        }
    }

    fn validate_use_tree(
        &mut self,
        tree: &syn::UseTree,
        prefix: &mut Vec<String>,
        use_item: &syn::ItemUse,
    ) {
        match tree {
            syn::UseTree::Path(p) => {
                prefix.push(p.ident.to_string());
                self.validate_use_tree(&p.tree, prefix, use_item);
                prefix.pop();
            }
            syn::UseTree::Name(n) => {
                let mut full_path = prefix.clone();
                full_path.push(n.ident.to_string());
                let use_vis = Vis::from_syn(&use_item.vis, &self.module_path);
                self.check_use_path(&full_path, n.ident.span(), &use_vis);
            }
            syn::UseTree::Rename(r) => {
                let mut full_path = prefix.clone();
                full_path.push(r.ident.to_string());
                let use_vis = Vis::from_syn(&use_item.vis, &self.module_path);
                self.check_use_path(&full_path, r.ident.span(), &use_vis);
            }
            syn::UseTree::Glob(_) => {
                if self.is_local_path(prefix) {
                    let outcome = self.glob_outcome(prefix);
                    if outcome == GlobOutcome::Private {
                        let span = use_item.use_token.span;
                        self.diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            file: self.file_path.to_path_buf(),
                            line: span.start().line,
                            column: span.start().column + 1,
                            message: format!("`{}` is private", prefix.join("::")),
                            error_code: Some("E0603".to_string()),
                            hint: None,
                            fix: None,
                        });
                    } else if outcome == GlobOutcome::Unresolved {
                        let span = use_item.use_token.span;
                        self.diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            file: self.file_path.to_path_buf(),
                            line: span.start().line,
                            column: span.start().column + 1,
                            message: format!(
                                "unresolved glob import `{}::*`",
                                prefix.join("::")
                            ),
                            error_code: Some("E0432".to_string()),
                            hint: None,
                            fix: None,
                        });
                    }
                } else {
                    self.report_unknown_crate(prefix, use_item.use_token.span);
                }
            }
            syn::UseTree::Group(g) => {
                for tree in &g.items {
                    self.validate_use_tree(tree, prefix, use_item);
                }
            }
        }
    }

    fn glob_outcome(&self, prefix: &[String]) -> GlobOutcome {
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

    fn check_reexport_visibility(
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

    /// A path is local when it starts with `crate`/`self`/`super`/the crate name,
    /// or with a child module of the current module.
    fn is_local_path(&self, path: &[String]) -> bool {
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
    fn is_unknown_external_crate(&self, prefix: &[String]) -> bool {
        if self.external_crates.is_none() {
            return false;
        }
        let Some(first) = prefix.first() else {
            return false;
        };
        !(self.is_known_crate(first)
            || self.macro_modules.contains(&self.module_path)
            || self.name_in_scope(first, prefix))
    }

    /// Reports a non-local `use` whose first segment is unknown. rustc: E0432 when the
    /// unknown crate is the last segment of the prefix, E0433 when more segments follow.
    fn report_unknown_crate(&mut self, prefix: &[String], span: proc_macro2::Span) {
        if !self.is_unknown_external_crate(prefix) {
            return;
        }
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
    fn is_known_crate(&self, name: &str) -> bool {
        is_sysroot_crate(name) || self.external_crates.is_some_and(|c| c.contains(name))
    }

    /// Whether `name` is brought into the current module or its enclosing blocks by an item,
    /// a `use`, or a glob that may bring any name. `skip_glob` is the glob being checked.
    fn name_in_scope(&self, name: &str, skip_glob: &[String]) -> bool {
        let symbols = self.symbols;
        let glob_may_bring = |prefix: &[String]| {
            if prefix == skip_glob {
                return false;
            }
            let target = symbols
                .resolve_use_path(&self.module_path, prefix, self.crate_name)
                .and_then(|abs| symbols.resolve_module(&abs, &self.module_path, self.crate_name));
            match target {
                Some(module) => !matches!(
                    symbols.resolve_item(&module.path, name, &self.module_path, self.crate_name),
                    Resolved::NotFound
                ),
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
        symbols
            .modules
            .values()
            .flat_map(|m| m.uses.iter())
            .filter(|u| u.module == self.module_path)
            .any(|u| if u.is_glob { glob_may_bring(&u.path) } else { u.alias == name })
    }

    /// Check if a `use` path resolves within the crate.
    fn check_use_path(&mut self, path: &[String], span: proc_macro2::Span, use_vis: &Vis) {
        if !self.is_local_path(path) {
            if path.len() == 2 {
                // Bare `Item::name` of an item defined in the current module
                let mut absolute = self.module_path.0.clone();
                absolute.extend(path.iter().cloned());
                if self.check_variant_use(path, &absolute, span, use_vis) {
                    return;
                }
            }
            if let Some((_, prefix)) = path.split_last() {
                self.report_unknown_crate(prefix, span);
            }
            return;
        }

        if path.len() < 2 {
            return;
        }

        // Resolve self::/super:: to absolute crate paths before lookup
        let symbols = self.symbols;
        let resolved = match symbols.resolve_use_path(&self.module_path, path, self.crate_name) {
            Some(r) => r,
            None if path[0] == "super" => {
                self.diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    file: self.file_path.to_path_buf(),
                    line: span.start().line,
                    column: span.start().column + 1,
                    message: format!(
                        "failed to resolve `{}`: there are too many leading `super` keywords",
                        path.join("::")
                    ),
                    error_code: Some("E0433".to_string()),
                    hint: None,
                    fix: None,
                });
                return;
            }
            None => return, // Can't resolve — likely external, skip
        };

        let item_name = &resolved[resolved.len() - 1];
        let module_segments = &resolved[..resolved.len() - 1];
        let resolved_module = symbols
            .resolve_module(module_segments, &self.module_path, self.crate_name)
            .filter(|m| symbols.module_exists(&m.path));

        if let Some(module) = resolved_module {
            let module_path = module.path;
            // `use foo::self` means "import the module itself"
            if item_name == "self" && module.accessible {
                return;
            }
            let (found, links) = symbols.resolve_item_links(
                &module_path,
                item_name,
                &self.module_path,
                self.crate_name,
            );

            if is_private_step(module.accessible, found) {
                self.diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    file: self.file_path.to_path_buf(),
                    line: span.start().line,
                    column: span.start().column + 1,
                    message: format!("`{}` is private", path.join("::")),
                    error_code: Some("E0603".to_string()),
                    hint: None,
                    fix: None,
                });
            } else if let Resolved::NotFound = found {
                // Find similar names for hint and potential fix
                let similar = self.find_similar_in_module(&module_path, item_name);
                let fix = similar.as_ref().and_then(|hint_msg| {
                    // Extract the suggested name from "did you mean `Foo`?"
                    let suggested = hint_msg
                        .strip_prefix("did you mean `")
                        .and_then(|s| s.strip_suffix("`?"));
                    suggested.map(|correct_name| {
                        Fix::ReplaceLine {
                            file: self.file_path.to_path_buf(),
                            line: span.start().line,
                            old_text: item_name.clone(),
                            new_text: correct_name.to_string(),
                        }
                    })
                });

                self.diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    file: self.file_path.to_path_buf(),
                    line: span.start().line,
                    column: span.start().column + 1,
                    message: format!("unresolved import `{}`", path.join("::")),
                    error_code: Some("E0432".to_string()),
                    hint: similar,
                    fix,
                });
            } else if let Resolved::Accessible(item) = found {
                let binds_value = match item.kind {
                    ItemKind::Function | ItemKind::Const | ItemKind::Static => true,
                    ItemKind::Struct => item.has_value_ctor,
                    _ => false,
                };
                self.check_reexport_visibility(item, &links, item_name, binds_value, use_vis, span);
            }
        } else if self.check_variant_use(path, &resolved, span, use_vis) {
        } else {
            self.diagnostics.push(Diagnostic {
                severity: Severity::Error,
                file: self.file_path.to_path_buf(),
                line: span.start().line,
                column: span.start().column + 1,
                message: format!("unresolved import `{}`", module_segments.join("::")),
                error_code: Some("E0432".to_string()),
                hint: None,
                fix: None,
            });
        }
    }

    /// Check a `use` whose path ends in `Enum::Variant`. Returns false when the parent
    /// of the last segment is not an enum of the crate, leaving the path to the caller.
    fn check_variant_use(
        &mut self,
        path: &[String],
        resolved: &[String],
        span: proc_macro2::Span,
        use_vis: &Vis,
    ) -> bool {
        let symbols = self.symbols;
        if resolved.len() < 3 {
            return false;
        }
        let variant_name = &resolved[resolved.len() - 1];
        let enum_name = &resolved[resolved.len() - 2];
        let parent_segments = &resolved[..resolved.len() - 2];
        let Some(parent) = symbols
            .resolve_module(parent_segments, &self.module_path, self.crate_name)
            .filter(|m| symbols.module_exists(&m.path))
        else {
            return false;
        };
        let (found, links) = symbols.resolve_item_links(
            &parent.path,
            enum_name,
            &self.module_path,
            self.crate_name,
        );
        let (message, code) = match found {
            Resolved::NotFound => return false,
            Resolved::Inaccessible(item) if item.kind == ItemKind::Enum => {
                (format!("enum `{}` is private", enum_name), "E0603")
            }
            Resolved::Inaccessible(_) => return false,
            Resolved::Accessible(item) => {
                if !parent.accessible && item.kind != ItemKind::Enum {
                    return false;
                }
                if item.kind == ItemKind::Trait {
                    // Trait members are not recorded, so a missing member (rustc: E0432)
                    // is not told apart from an existing one.
                    (
                        "`use` associated items of traits is unstable".to_string(),
                        "E0658",
                    )
                } else if matches!(item.kind, ItemKind::Macro | ItemKind::Module) {
                    return false;
                } else if item.kind != ItemKind::Enum {
                    (
                        format!("unresolved import `{}`", path[..path.len() - 1].join("::")),
                        "E0432",
                    )
                } else if !parent.accessible {
                    (format!("enum `{}` is private", enum_name), "E0603")
                } else if let Some(variant) = item.variants.iter().find(|v| &v.name == variant_name) {
                    self.check_reexport_visibility(
                        item,
                        &links,
                        variant_name,
                        !variant.is_named_fields,
                        use_vis,
                        span,
                    );
                    return true;
                } else {
                    (format!("unresolved import `{}`", path.join("::")), "E0432")
                }
            }
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
        true
    }

    /// Walk the AST checking type references, field access, function calls, etc.
    fn validate_references(&mut self, ast: &syn::File) {
        let mut ref_visitor = RefVisitor {
            validator: self,
        };
        syn::visit::visit_file(&mut ref_visitor, ast);
    }

    /// Find similar item names in a module for "did you mean?" suggestions.
    fn find_similar_in_module(&self, module: &ModulePath, name: &str) -> Option<String> {
        let module_info = self.symbols.modules.get(module)?;
        let similar: Vec<_> = module_info
            .items
            .iter()
            .filter(|i| is_similar(&i.name, name))
            .collect();

        if similar.len() == 1 {
            Some(format!("did you mean `{}`?", similar[0].name))
        } else {
            None
        }
    }

    /// Resolve a name visible in the current module (own items, uses, re-exports, globs).
    fn lookup_in_scope(&self, name: &str) -> Option<&ItemInfo> {
        if let LocalLookup::Item(item) = self.lookup_local(name) {
            return Some(item);
        }
        self.symbols
            .resolve_item(&self.module_path, name, &self.module_path, self.crate_name)
            .accessible()
    }

    /// Names brought in by `use` items of the enclosing blocks, innermost first.
    fn lookup_local(&self, name: &str) -> LocalLookup<'_> {
        let symbols = self.symbols;
        let mut imported = false;
        for scope in self.local_scopes.iter().rev() {
            for import in scope.imports.iter().filter(|i| i.name == name) {
                imported = true;
                let item = symbols
                    .resolve_use_path(&self.module_path, &import.path, self.crate_name)
                    .and_then(|abs| {
                        let (item_name, module_part) = abs.split_last()?;
                        let module =
                            symbols.resolve_module(module_part, &self.module_path, self.crate_name)?;
                        symbols
                            .resolve_item(&module.path, item_name, &self.module_path, self.crate_name)
                            .accessible()
                    });
                if let Some(item) = item {
                    return LocalLookup::Item(item);
                }
            }
            for prefix in &scope.globs {
                let target = symbols
                    .resolve_use_path(&self.module_path, prefix, self.crate_name)
                    .and_then(|abs| {
                        symbols.resolve_module(&abs, &self.module_path, self.crate_name)
                    });
                match target {
                    Some(module) => {
                        if let Some(item) = symbols
                            .resolve_item(&module.path, name, &self.module_path, self.crate_name)
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

    /// Find an item by name across the entire crate (for suggestions).
    fn find_anywhere(&self, name: &str) -> Vec<(&ModulePath, &ItemInfo)> {
        let mut results = Vec::new();
        for (mod_path, module_info) in &self.symbols.modules {
            for item in &module_info.items {
                if item.name == name {
                    results.push((mod_path, item));
                }
            }
        }
        results
    }
}

/// The privacy rule shared by `use` paths and expression paths: a path is private when the
/// module it goes through, or the item it names, is not visible to the accessor.
fn is_private_step(module_accessible: bool, found: Resolved) -> bool {
    !module_accessible || matches!(found, Resolved::Inaccessible(_))
}

/// Visitor that walks expressions looking for references to validate.
struct RefVisitor<'a, 'b> {
    validator: &'a mut ValidationVisitor<'b>,
}

impl<'a, 'b, 'ast> Visit<'ast> for RefVisitor<'a, 'b> {
    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        if node.qself.is_none() {
            self.check_path_modules(&node.path);
        }
        let type_name = path_last_segment(&node.path);
        if let Some(type_name) = type_name {
            let struct_fields: Option<Vec<String>> = {
                self.validator.lookup_in_scope(&type_name)
                    .filter(|i| i.kind == ItemKind::Struct)
                    .map(|item| item.fields.iter().map(|f| f.name.clone()).collect())
            };

            if let Some(expected_fields) = struct_fields {
                if node.rest.is_none() {
                    let provided: Vec<String> = node
                        .fields
                        .iter()
                        .filter_map(|f| {
                            if let syn::Member::Named(ident) = &f.member {
                                Some(ident.to_string())
                            } else {
                                None
                            }
                        })
                        .collect();

                    for field_name in &expected_fields {
                        if !provided.contains(field_name) {
                            let span = node.path.segments.last().map(|s| s.ident.span())
                                .unwrap_or_else(proc_macro2::Span::call_site);
                            self.validator.diagnostics.push(Diagnostic {
                                severity: Severity::Error,
                                file: self.validator.file_path.to_path_buf(),
                                line: span.start().line,
                                column: span.start().column + 1,
                                message: format!(
                                    "missing field `{field_name}` in initializer of `{type_name}`"
                                ),
                                error_code: Some("E0063".to_string()),
                                hint: None,
                                fix: None,
                            });
                        }
                    }

                    for provided_name in &provided {
                        if !expected_fields.contains(provided_name) {
                            let span = node.fields.iter()
                                .find(|f| {
                                    if let syn::Member::Named(ident) = &f.member {
                                        ident == provided_name
                                    } else {
                                        false
                                    }
                                })
                                .map(|f| {
                                    if let syn::Member::Named(ident) = &f.member {
                                        ident.span()
                                    } else {
                                        proc_macro2::Span::call_site()
                                    }
                                })
                                .unwrap_or_else(proc_macro2::Span::call_site);

                            self.validator.diagnostics.push(Diagnostic {
                                severity: Severity::Error,
                                file: self.validator.file_path.to_path_buf(),
                                line: span.start().line,
                                column: span.start().column + 1,
                                message: format!(
                                    "struct `{type_name}` has no field named `{provided_name}`"
                                ),
                                error_code: Some("E0609".to_string()),
                                hint: None,
                                fix: None,
                            });
                        }
                    }
                }
            }
        }

        syn::visit::visit_expr_struct(self, node);
    }

    fn visit_expr_field(&mut self, node: &'ast syn::ExprField) {
        syn::visit::visit_expr_field(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*node.func {
            if let Some(fn_name) = path_last_segment(&path.path) {
                if let Some(item) = self.validator.lookup_in_scope(&fn_name)
                    .filter(|i| i.kind == ItemKind::Function)
                {
                    if let Some(expected) = item.param_count {
                        let actual = node.args.len();
                        if actual != expected {
                            let span = path.path.segments.last().map(|s| s.ident.span())
                                .unwrap_or_else(proc_macro2::Span::call_site);
                            self.validator.diagnostics.push(Diagnostic {
                                severity: Severity::Error,
                                file: self.validator.file_path.to_path_buf(),
                                line: span.start().line,
                                column: span.start().column + 1,
                                message: format!(
                                    "function `{fn_name}` takes {expected} argument(s) but {actual} were supplied"
                                ),
                                error_code: Some("E0061".to_string()),
                                hint: None,
                                fix: None,
                            });
                        }
                    }
                }
            }
        }

        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if node.qself.is_none() {
            self.check_path_modules(&node.path);
        }

        // Check enum variant paths like `Foo::Bar`
        if node.path.segments.len() == 2 {
            let type_name = node.path.segments[0].ident.to_string();
            let variant_name = node.path.segments[1].ident.to_string();

            let check_result: Option<(Vec<String>, Vec<String>)> = {
                self.validator.lookup_in_scope(&type_name)
                    .filter(|i| i.kind == ItemKind::Enum)
                    .map(|item| {
                        let variants: Vec<String> = item.variants.iter().map(|v| v.name.clone()).collect();
                        let similar: Vec<String> = item.variants.iter()
                            .filter(|v| is_similar(&v.name, &variant_name))
                            .map(|v| v.name.clone())
                            .collect();
                        (variants, similar)
                    })
            };

            if let Some((variants, similar)) = check_result {
                if !variants.contains(&variant_name) {
                    let has_method = self.validator.symbols
                        .find_methods(&type_name)
                        .iter()
                        .any(|(_, m)| m.name == variant_name);

                    if !has_method {
                        let span = node.path.segments[1].ident.span();
                        self.validator.diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            file: self.validator.file_path.to_path_buf(),
                            line: span.start().line,
                            column: span.start().column + 1,
                            message: format!(
                                "no variant `{variant_name}` in enum `{type_name}`"
                            ),
                            error_code: Some("E0599".to_string()),
                            hint: if similar.len() == 1 {
                                Some(format!("did you mean `{}`?", similar[0]))
                            } else {
                                None
                            },
                            fix: None,
                        });
                    }
                }
            }
        }

        // Check for unresolved types/paths that exist elsewhere in the crate
        if node.path.segments.len() == 1 {
            let name = node.path.segments[0].ident.to_string();
            if name.starts_with(char::is_uppercase) {
                self.check_type_in_scope(&name, node.path.segments[0].ident.span());
            }
        }

        syn::visit::visit_expr_path(self, node);
    }

    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        if node.qself.is_none() {
            self.check_path_modules(&node.path);
        }

        if node.qself.is_none() && node.path.segments.len() == 1 {
            let name = node.path.segments[0].ident.to_string();
            if name.starts_with(char::is_uppercase) {
                self.check_type_in_scope(&name, node.path.segments[0].ident.span());
            }
        }

        syn::visit::visit_type_path(self, node);
    }

    fn visit_item_use(&mut self, _node: &'ast syn::ItemUse) {
        // Module-level uses are handled in validate_use_statements, block uses in visit_block
    }

    fn visit_block(&mut self, node: &'ast syn::Block) {
        let mut scope = LocalScope::default();
        for stmt in &node.stmts {
            if let syn::Stmt::Item(syn::Item::Use(u)) = stmt {
                collect_local_imports(&u.tree, &mut Vec::new(), &mut scope);
            }
        }
        self.validator.local_scopes.push(scope);
        for stmt in &node.stmts {
            if let syn::Stmt::Item(syn::Item::Use(u)) = stmt {
                self.validator.validate_use_tree(&u.tree, &mut Vec::new(), u);
            }
        }
        syn::visit::visit_block(self, node);
        self.validator.local_scopes.pop();
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if node.content.is_none() {
            return;
        }
        let inner = self.validator.module_path.child(&node.ident.to_string());
        let outer = std::mem::replace(&mut self.validator.module_path, inner);
        syn::visit::visit_item_mod(self, node);
        self.validator.module_path = outer;
    }
}

impl<'a, 'b> RefVisitor<'a, 'b> {
    /// E0433 for an expression or type path whose module part names nothing in the crate.
    fn check_path_modules(&mut self, path: &syn::Path) {
        let names: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
        if names.len() < 2 || !self.validator.is_local_path(&names) {
            return;
        }
        let symbols = self.validator.symbols;
        let Some(abs) = symbols.resolve_use_path(
            &self.validator.module_path,
            &names,
            self.validator.crate_name,
        ) else {
            return;
        };
        let skipped = names.len() as isize - abs.len() as isize;
        let Some(last) = abs.len().checked_sub(1) else {
            return;
        };
        let walk = symbols.walk_path(
            &abs[..last],
            &self.validator.module_path,
            self.validator.crate_name,
        );
        let stop_index = match walk.stop {
            Some(PathStop::Unrooted) => return,
            Some(PathStop::Missing { index } | PathStop::NotModule { index }) => Some(index),
            None => None,
        };
        let private_index = walk
            .first_private
            .filter(|private| stop_index.is_none_or(|stop| *private <= stop));
        if let Some(index) = private_index {
            self.report_private(path, (index as isize + skipped) as usize, &abs[index]);
            return;
        }
        match walk.stop {
            Some(PathStop::Missing { index }) => {
                let span = path.segments[(index as isize + skipped) as usize].ident.span();
                self.validator.diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    file: self.validator.file_path.to_path_buf(),
                    line: span.start().line,
                    column: span.start().column + 1,
                    message: format!(
                        "failed to resolve: could not find `{}` in `{}`",
                        abs[index],
                        abs[index - 1]
                    ),
                    error_code: Some("E0433".to_string()),
                    hint: None,
                    fix: None,
                });
                return;
            }
            Some(_) => return,
            None => {}
        }
        let found = symbols.resolve_item(
            &walk.reached,
            &abs[last],
            &self.validator.module_path,
            self.validator.crate_name,
        );
        if is_private_step(true, found) {
            self.report_private(path, (last as isize + skipped) as usize, &abs[last]);
        }
    }

    fn report_private(&mut self, path: &syn::Path, segment_index: usize, name: &str) {
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

    fn private_behind_glob(&self, name: &str) -> Option<String> {
        let v = &*self.validator;
        let symbols = v.symbols;
        let block_globs = v.local_scopes.iter().flat_map(|s| s.globs.iter().map(Vec::as_slice));
        let module_globs = symbols.glob_paths(&v.module_path);
        block_globs.chain(module_globs).find_map(|prefix| {
            let abs = symbols.resolve_use_path(&v.module_path, prefix, v.crate_name)?;
            let target = symbols.resolve_module(&abs, &v.module_path, v.crate_name)?;
            match symbols.resolve_item(&target.path, name, &v.module_path, v.crate_name) {
                Resolved::Inaccessible(_) => Some(target.path.display()),
                _ => None,
            }
        })
    }

    /// Check if a type name is in scope; if not, suggest where it lives in the crate.
    /// Generates a Fix (insert use statement) when there's a single unambiguous candidate,
    /// or uses smart import resolution when there are multiple candidates.
    fn check_type_in_scope(&mut self, name: &str, span: proc_macro2::Span) {
        let in_scope = !matches!(self.validator.lookup_local(name), LocalLookup::None)
            || self.validator.lookup_in_scope(name).is_some();

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
            return; // Not in crate — could be external, stay quiet
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
            error_code: Some("E0412".to_string()),
            hint: Some(hint),
            fix,
        });
    }
}

#[derive(Default)]
struct LocalScope {
    imports: Vec<LocalImport>,
    globs: Vec<Vec<String>>,
}

struct LocalImport {
    name: String,
    path: Vec<String>,
}

enum LocalLookup<'a> {
    Item(&'a ItemInfo),
    Imported,
    None,
}

fn collect_local_imports(tree: &syn::UseTree, prefix: &mut Vec<String>, scope: &mut LocalScope) {
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

/// Check if a path starts with `crate`, `self`, `super`, or the crate's own name.
fn reexport_message(name: &str, item_scope: &ModulePath) -> String {
    if *item_scope == ModulePath::root() {
        format!("`{name}` is only public within the crate, and cannot be re-exported outside")
    } else {
        format!("`{name}` is private, and cannot be re-exported")
    }
}

/// Sysroot crates in the extern prelude of every crate; `alloc` and `test` need
/// `extern crate`.
fn is_sysroot_crate(name: &str) -> bool {
    matches!(name, "std" | "core" | "proc_macro")
}

fn is_crate_path(path: &[String], crate_name: Option<&str>) -> bool {
    path.first().is_some_and(|s| {
        s == "crate" || s == "self" || s == "super"
            || crate_name.is_some_and(|name| name == s)
    })
}

/// Get the last segment name from a path.
fn path_last_segment(path: &syn::Path) -> Option<String> {
    path.segments.last().map(|s| s.ident.to_string())
}

/// Levenshtein edit distance between two strings.
fn levenshtein(a: &str, b: &str) -> usize {
    let a_len = a.len();
    let b_len = b.len();

    if a_len == 0 { return b_len; }
    if b_len == 0 { return a_len; }

    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr = vec![0; b_len + 1];

    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j] + cost)
                .min(prev[j + 1] + 1)
                .min(curr[j] + 1);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b_len]
}

/// Check if two names are similar enough to suggest one for the other.
/// Uses case-insensitive Levenshtein distance, scaled by name length.
fn is_similar(a: &str, b: &str) -> bool {
    if a == b {
        return false; // exact match isn't "similar", it's the same
    }

    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();

    // Case-insensitive exact match
    if a_lower == b_lower {
        return true;
    }

    let dist = levenshtein(&a_lower, &b_lower);
    let max_len = a.len().max(b.len());

    // Allow distance proportional to length:
    //   len 1-4: distance <= 1
    //   len 5-8: distance <= 2
    //   len 9+:  distance <= 3
    let threshold = match max_len {
        0..=4 => 1,
        5..=8 => 2,
        _ => 3,
    };

    dist <= threshold
}
