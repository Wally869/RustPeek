use super::*;

impl<'a, 'b, 'ast> Visit<'ast> for RefVisitor<'a, 'b> {
    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        if node.qself.is_none() {
            self.check_path_modules(&node.path);
        }
        let type_name = path_last_segment(&node.path).filter(|_| node.qself.is_none());
        let names: Vec<String> = node.path.segments.iter().map(|s| s.ident.to_string()).collect();
        if let Some(type_name) = type_name {
            // Through its own path: `Enum::Variant { .. }` names no struct.
            let struct_fields: Option<Vec<String>> = {
                self.validator.lookup_path(&names)
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
            let names: Vec<String> =
                path.path.segments.iter().map(|s| s.ident.to_string()).collect();
            if let Some(fn_name) = path_last_segment(&path.path).filter(|_| path.qself.is_none()) {
                if let Some(item) = self.validator.lookup_path(&names)
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

        // `Type::name` on a local enum or struct: reported only when no impl, derive,
        // blanket impl or macro could provide `name`.
        if node.path.segments.len() == 2 {
            let type_name = node.path.segments[0].ident.to_string();
            let member_name = node.path.segments[1].ident.to_string();

            let check_result: Option<(bool, String, Vec<String>)> = {
                self.validator.lookup_in_scope(&type_name)
                    .filter(|i| matches!(i.kind, ItemKind::Enum | ItemKind::Struct))
                    .filter(|i| !i.variants.iter().any(|v| v.name == member_name))
                    .map(|item| {
                        let similar: Vec<String> = item.variants.iter()
                            .filter(|v| is_similar(&v.name, &member_name))
                            .map(|v| v.name.clone())
                            .collect();
                        (item.kind == ItemKind::Enum, item.name.clone(), similar)
                    })
            };

            // Impls name the type by its own name; the path may use an alias (`use a::T as U`).
            if let Some((is_enum, item_name, similar)) = check_result {
                let symbols = self.validator.symbols;
                if !symbols.may_have_associated_item(&item_name, &member_name)
                    && !symbols.may_have_associated_item(&type_name, &member_name)
                {
                    let span = node.path.segments[1].ident.span();
                    let message = if is_enum {
                        format!("no variant or associated item named `{member_name}` found for enum `{type_name}` in the current scope")
                    } else {
                        format!("no function or associated item named `{member_name}` found for struct `{type_name}` in the current scope")
                    };
                    self.validator.diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        file: self.validator.file_path.to_path_buf(),
                        line: span.start().line,
                        column: span.start().column + 1,
                        message,
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

        // Check for unresolved types/paths that exist elsewhere in the crate
        if node.path.segments.len() == 1 {
            let name = node.path.segments[0].ident.to_string();
            self.check_name_in_scope(&name, node.path.segments[0].ident.span());
        }

        syn::visit::visit_expr_path(self, node);
    }

    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        if node.qself.is_none() {
            self.check_path_modules(&node.path);
        }

        if node.qself.is_none() && node.path.segments.len() == 1 {
            let name = node.path.segments[0].ident.to_string();
            self.check_name_in_scope(&name, node.path.segments[0].ident.span());
        }

        syn::visit::visit_type_path(self, node);
    }

    fn visit_item_use(&mut self, _node: &'ast syn::ItemUse) {
        // Module-level uses are handled in validate_use_statements, block uses in visit_block
    }

    fn visit_block(&mut self, node: &'ast syn::Block) {
        let mut scope = LocalScope::default();
        for stmt in &node.stmts {
            match stmt {
                syn::Stmt::Item(item) => {
                    if let syn::Item::Use(u) = item {
                        collect_local_imports(&u.tree, &mut Vec::new(), &mut scope);
                    }
                    scope.names.extend(declared_item_name(item));
                    scope.has_macro_items |=
                        matches!(item, syn::Item::Macro(m) if m.ident.is_none());
                }
                syn::Stmt::Local(local) => scope.names.extend(pattern_names(&local.pat)),
                // Brace-delimited, as item macros are; `println!(..);` defines nothing.
                syn::Stmt::Macro(m) => {
                    scope.has_macro_items |=
                        matches!(m.mac.delimiter, syn::MacroDelimiter::Brace(_));
                }
                _ => {}
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

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.with_names(signature_names(&node.sig), |v| syn::visit::visit_item_fn(v, node));
    }

    fn visit_arm(&mut self, node: &'ast syn::Arm) {
        self.with_names(pattern_names(&node.pat), |v| syn::visit::visit_arm(v, node));
    }

    fn visit_expr_closure(&mut self, node: &'ast syn::ExprClosure) {
        let names = node.inputs.iter().flat_map(pattern_names).collect();
        self.with_names(names, |v| syn::visit::visit_expr_closure(v, node));
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.with_names(pattern_names(&node.pat), |v| syn::visit::visit_expr_for_loop(v, node));
    }

    fn visit_expr_let(&mut self, node: &'ast syn::ExprLet) {
        if let Some(scope) = self.validator.local_scopes.last_mut() {
            scope.names.extend(pattern_names(&node.pat));
        }
        syn::visit::visit_expr_let(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        self.with_generics(&node.generics, |v| syn::visit::visit_item_impl(v, node));
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        self.with_generics(&node.generics, |v| syn::visit::visit_item_struct(v, node));
    }

    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        self.with_generics(&node.generics, |v| syn::visit::visit_item_enum(v, node));
    }

    fn visit_item_union(&mut self, node: &'ast syn::ItemUnion) {
        self.with_generics(&node.generics, |v| syn::visit::visit_item_union(v, node));
    }

    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        self.with_generics(&node.generics, |v| syn::visit::visit_item_trait(v, node));
    }

    fn visit_item_type(&mut self, node: &'ast syn::ItemType) {
        self.with_generics(&node.generics, |v| syn::visit::visit_item_type(v, node));
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.with_names(signature_names(&node.sig), |v| syn::visit::visit_impl_item_fn(v, node));
    }

    fn visit_impl_item_type(&mut self, node: &'ast syn::ImplItemType) {
        self.with_generics(&node.generics, |v| syn::visit::visit_impl_item_type(v, node));
    }

    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        self.with_names(signature_names(&node.sig), |v| syn::visit::visit_trait_item_fn(v, node));
    }

    fn visit_trait_item_type(&mut self, node: &'ast syn::TraitItemType) {
        self.with_generics(&node.generics, |v| syn::visit::visit_trait_item_type(v, node));
    }
}
