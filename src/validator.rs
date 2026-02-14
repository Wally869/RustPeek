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
) -> Vec<Diagnostic> {
    let mut visitor = ValidationVisitor {
        diagnostics: Vec::new(),
        file_path,
        module_path,
        symbols,
        src_dir,
        source_lines: None,
        crate_name,
    };

    visitor.validate_mod_declarations(ast);
    visitor.validate_use_statements(ast);
    visitor.validate_references(ast);

    visitor.diagnostics
}

struct ValidationVisitor<'a> {
    diagnostics: Vec<Diagnostic>,
    file_path: &'a Path,
    module_path: &'a ModulePath,
    symbols: &'a SymbolTable,
    src_dir: &'a Path,
    /// Lazily loaded source lines for fix generation
    source_lines: Option<Vec<String>>,
    /// The crate's own name (from Cargo.toml), so `use <name>::...` is treated as `use crate::...`
    crate_name: Option<&'a str>,
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
                    discovery::resolve_mod_file(self.src_dir, self.module_path, &mod_name);

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
        for item in &ast.items {
            if let syn::Item::Use(u) = item {
                self.validate_use_tree(&u.tree, &mut Vec::new(), u);
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
                self.check_use_path(&full_path, n.ident.span());
            }
            syn::UseTree::Rename(r) => {
                let mut full_path = prefix.clone();
                full_path.push(r.ident.to_string());
                self.check_use_path(&full_path, r.ident.span());
            }
            syn::UseTree::Glob(_) => {
                if is_crate_path(prefix, self.crate_name) {
                    let mod_path = ModulePath(prefix.clone());
                    if !self.symbols.modules.contains_key(&mod_path) {
                        let parent_path = ModulePath(prefix[..prefix.len() - 1].to_vec());
                        let last = prefix.last().map(|s| s.as_str()).unwrap_or("");
                        if self.symbols.find_in_module(&parent_path, last).is_none() {
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
                    }
                }
            }
            syn::UseTree::Group(g) => {
                for tree in &g.items {
                    self.validate_use_tree(tree, prefix, use_item);
                }
            }
        }
    }

    /// Check if a `use` path resolves within the crate.
    fn check_use_path(&mut self, path: &[String], span: proc_macro2::Span) {
        if !is_crate_path(path, self.crate_name) {
            return;
        }

        if path.len() < 2 {
            return;
        }

        // Resolve self::/super:: to absolute crate paths before lookup
        let resolved = match self.resolve_use_path(path) {
            Some(r) => r,
            None => return, // Can't resolve — likely external, skip
        };

        let item_name = &resolved[resolved.len() - 1];
        let module_segments = &resolved[..resolved.len() - 1];
        let module_path = ModulePath(module_segments.to_vec());

        // `use foo::self` means "import the module itself" — valid if the module exists
        if item_name == "self" {
            if self.symbols.modules.contains_key(&module_path) {
                return;
            }
            // Fall through to "unresolved module" error below
        }

        if let Some(module_info) = self.symbols.modules.get(&module_path) {
            let item_exists = module_info.items.iter().any(|i| i.name == *item_name)
                || module_info.uses.iter().any(|u| {
                    !u.is_glob && u.vis != Vis::Private && u.alias == *item_name
                });

            if !item_exists {
                let child_mod = module_path.child(item_name);
                if self.symbols.modules.contains_key(&child_mod) {
                    return;
                }

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
            } else {
                // Item exists — check visibility
                if let Some(item) = module_info.items.iter().find(|i| i.name == *item_name) {
                    if !item.vis.accessible_from(&item.module, self.module_path) {
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
                    }
                }
            }
        } else {
            self.diagnostics.push(Diagnostic {
                severity: Severity::Error,
                file: self.file_path.to_path_buf(),
                line: span.start().line,
                column: span.start().column + 1,
                message: format!("unresolved module `{}`", module_segments.join("::")),
                error_code: Some("E0433".to_string()),
                hint: None,
                fix: None,
            });
        }
    }

    /// Walk the AST checking type references, field access, function calls, etc.
    fn validate_references(&mut self, ast: &syn::File) {
        let mut ref_visitor = RefVisitor {
            validator: self,
        };
        syn::visit::visit_file(&mut ref_visitor, ast);
    }

    /// Resolve a use path to an absolute crate path.
    fn resolve_use_path(&self, path: &[String]) -> Option<Vec<String>> {
        if path.is_empty() {
            return None;
        }

        let first = path[0].as_str();

        // Treat the crate's own name as `crate`
        let is_own_name = self.crate_name.is_some_and(|name| name == first);

        if first == "crate" || is_own_name {
            if is_own_name {
                // Replace crate name with "crate" for uniform lookup
                let mut resolved = vec!["crate".to_string()];
                resolved.extend(path[1..].iter().cloned());
                Some(resolved)
            } else {
                Some(path.to_vec())
            }
        } else {
            match first {
                "self" => {
                    let mut resolved = self.module_path.0.clone();
                    resolved.extend(path[1..].iter().cloned());
                    Some(resolved)
                }
                "super" => {
                    if let Some(parent) = self.module_path.parent() {
                        let mut resolved = parent.0;
                        resolved.extend(path[1..].iter().cloned());
                        Some(resolved)
                    } else {
                        None
                    }
                }
                _ => {
                    let candidate = self.module_path.child(first);
                    if self.symbols.modules.contains_key(&candidate) {
                        let mut resolved = candidate.0;
                        resolved.extend(path[1..].iter().cloned());
                        return Some(resolved);
                    }
                    None
                }
            }
        }
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

    /// Resolve what names are in scope for a given module.
    fn names_in_scope(&self) -> Vec<(String, &ItemInfo)> {
        let mut scope = Vec::new();

        if let Some(module_info) = self.symbols.modules.get(self.module_path) {
            for item in &module_info.items {
                scope.push((item.name.clone(), item));
            }

            for use_info in &module_info.uses {
                let resolved = self.resolve_use_path(&use_info.path);

                if use_info.is_glob {
                    if let Some(resolved) = &resolved {
                        let mod_path = ModulePath(resolved.clone());
                        if let Some(imported_mod) = self.symbols.modules.get(&mod_path) {
                            for item in &imported_mod.items {
                                if item.vis.accessible_from(&item.module, self.module_path) {
                                    scope.push((item.name.clone(), item));
                                }
                            }
                        }
                    }
                } else {
                    if let Some(resolved) = &resolved {
                        if resolved.len() >= 2 {
                            let item_name = resolved.last().unwrap();
                            let mod_path = ModulePath(resolved[..resolved.len() - 1].to_vec());
                            if let Some(item) = self.symbols.find_in_module(&mod_path, item_name) {
                                scope.push((use_info.alias.clone(), item));
                            }
                        }
                    }
                }
            }
        }

        scope
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

/// Visitor that walks expressions looking for references to validate.
struct RefVisitor<'a, 'b> {
    validator: &'a mut ValidationVisitor<'b>,
}

impl<'a, 'b, 'ast> Visit<'ast> for RefVisitor<'a, 'b> {
    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        let type_name = path_last_segment(&node.path);
        if let Some(type_name) = type_name {
            let struct_fields: Option<Vec<String>> = {
                let scope = self.validator.names_in_scope();
                scope.iter()
                    .find(|(n, i)| *n == type_name && i.kind == ItemKind::Struct)
                    .map(|(_, item)| item.fields.iter().map(|f| f.name.clone()).collect())
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
                let scope = self.validator.names_in_scope();
                if let Some((_, item)) = scope.iter().find(|(n, i)| {
                    *n == fn_name && i.kind == ItemKind::Function
                }) {
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
        // Check enum variant paths like `Foo::Bar`
        if node.path.segments.len() == 2 {
            let type_name = node.path.segments[0].ident.to_string();
            let variant_name = node.path.segments[1].ident.to_string();

            let check_result: Option<(Vec<String>, Vec<String>)> = {
                let scope = self.validator.names_in_scope();
                scope.iter()
                    .find(|(n, i)| *n == type_name && i.kind == ItemKind::Enum)
                    .map(|(_, item)| {
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
        if node.qself.is_none() && node.path.segments.len() == 1 {
            let name = node.path.segments[0].ident.to_string();
            if name.starts_with(char::is_uppercase) {
                self.check_type_in_scope(&name, node.path.segments[0].ident.span());
            }
        }

        syn::visit::visit_type_path(self, node);
    }

    fn visit_item_use(&mut self, _node: &'ast syn::ItemUse) {
        // Already handled in validate_use_statements
    }
}

impl<'a, 'b> RefVisitor<'a, 'b> {
    /// Check if a type name is in scope; if not, suggest where it lives in the crate.
    /// Generates a Fix (insert use statement) when there's a single unambiguous candidate,
    /// or uses smart import resolution when there are multiple candidates.
    fn check_type_in_scope(&mut self, name: &str, span: proc_macro2::Span) {
        let in_scope = {
            let scope = self.validator.names_in_scope();
            scope.iter().any(|(n, _)| *n == name)
        };

        if in_scope {
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

/// Check if a path starts with `crate`, `self`, `super`, or the crate's own name.
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
