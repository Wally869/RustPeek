use super::*;

/// Check if two names are similar enough to suggest one for the other.
/// Uses case-insensitive Levenshtein distance, scaled by name length.
pub(super) fn is_similar(a: &str, b: &str) -> bool {
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

impl<'a> ValidationVisitor<'a> {
    /// Find the best line to insert a `use` statement in the current file.
    /// Returns the line number to insert BEFORE (1-indexed).
    pub(super) fn find_use_insert_line(&mut self) -> usize {
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
    pub(super) fn validate_mod_declarations(&mut self, ast: &syn::File) {
        for item in &ast.items {
            if let syn::Item::Mod(m) = item {
                if m.content.is_some() {
                    continue;
                }

                let mod_name = m.ident.to_string();
                let resolved =
                    discovery::resolve_mod_file(self.file_path, self.owned_dir, false, m);

                if resolved.is_none() && !self.has_cfg_twin(&ast.items, m) {
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
    pub(super) fn validate_use_statements(&mut self, ast: &syn::File) {
        self.validate_use_items(&ast.items);
    }

    pub(super) fn validate_use_items(&mut self, items: &[syn::Item]) {
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

    pub(super) fn validate_use_tree(
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

    /// Whether `module` may bring `name` through something the symbol table cannot resolve
    /// to an item: an import of that name, or a glob reaching outside the crate.
    pub(super) fn module_may_hide(&self, module: &ModulePath, name: &str, seen: &mut Vec<ModulePath>) -> bool {
        if seen.contains(module) {
            return false;
        }
        seen.push(module.clone());
        let symbols = self.symbols;
        let Some(info) = symbols.modules.get(module).or_else(|| symbols.enclosing_file_module(module))
        else {
            return true;
        };
        if self.symbols.has_macro_call(module) {
            return true;
        }
        info.uses.iter().filter(|u| u.module == *module).any(|u| {
            if !u.is_glob {
                return u.alias == name;
            }
            let target = symbols
                .resolve_use_path(module, &u.path, self.crate_name)
                .and_then(|abs| symbols.walk_path(&abs, module, self.crate_name).module());
            match target {
                Some(target) => self.module_may_hide(&target.path, name, seen),
                None => true,
            }
        })
    }

    /// The `use` items of the current module, including inline modules' own.
    pub(super) fn module_uses(&self) -> impl Iterator<Item = &UseInfo> {
        self.symbols
            .modules
            .values()
            .flat_map(|m| m.uses.iter())
            .filter(|u| u.module == self.module_path)
    }

    /// Walk the AST checking type references, field access, function calls, etc.
    pub(super) fn validate_references(&mut self, ast: &syn::File) {
        let mut ref_visitor = RefVisitor {
            validator: self,
        };
        syn::visit::visit_file(&mut ref_visitor, ast);
    }

    /// Find similar item names in a module for "did you mean?" suggestions.
    pub(super) fn find_similar_in_module(&self, module: &ModulePath, name: &str) -> Option<String> {
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

    /// Find an item by name across the entire crate (for suggestions).
    pub(super) fn find_anywhere(&self, name: &str) -> Vec<(&ModulePath, &ItemInfo)> {
        let mut results = Vec::new();
        // `item.module`, not the file's module: an item of an inline `mod` lives in its
        // file's module info, tagged with the inline module path.
        for module_info in self.symbols.modules.values() {
            for item in &module_info.items {
                if item.name == name {
                    results.push((&item.module, item));
                }
            }
        }
        results
    }

}
