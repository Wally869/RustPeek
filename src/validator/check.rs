use super::*;

impl<'a> ValidationVisitor<'a> {
    /// Check if a `use` path resolves within the crate.
    pub(super) fn check_use_path(&mut self, path: &[String], span: proc_macro2::Span, use_vis: &Vis) {
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
        let module_walk = symbols.walk_path(module_segments, &self.module_path, self.crate_name);
        if matches!(module_walk.stop, Some(PathStop::External { .. })) {
            return;
        }
        let resolved_module = module_walk.module().filter(|m| symbols.module_exists(&m.path));

        if let Some(module) = resolved_module {
            let module_path = module.path;
            // `use foo::self` means "import the module itself"
            if item_name == "self" && module.accessible {
                return;
            }
            let (found, links) = symbols.resolve_item_links(
                &module_path,
                item_name,
                Namespace::Any,
                &self.module_path,
                self.crate_name,
            );

            if matches!(found, Resolved::External) {
                return;
            }
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
    pub(super) fn check_variant_use(
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
            .walk_path(parent_segments, &self.module_path, self.crate_name)
            .module()
            .filter(|m| symbols.module_exists(&m.path))
        else {
            return false;
        };
        let (found, links) = symbols.resolve_item_links(
            &parent.path,
            enum_name,
            Namespace::Any,
            &self.module_path,
            self.crate_name,
        );
        let (message, code) = match found {
            Resolved::NotFound | Resolved::External => return false,
            Resolved::Inaccessible(item) if item.kind == ItemKind::Enum => {
                (format!("enum `{}` is private", enum_name), "E0603")
            }
            Resolved::Inaccessible(_) => return false,
            Resolved::Accessible(item) => {
                if !parent.accessible && item.kind != ItemKind::Enum {
                    return false;
                }
                if item.kind == ItemKind::Trait {
                    // rustc: E0658 for an existing member, E0432 for a missing one.
                    if item.has_macro_items && !item.fields.iter().any(|m| &m.name == variant_name) {
                        // A macro call in the trait body may define the member.
                        return false;
                    }
                    if item.fields.iter().any(|m| &m.name == variant_name) {
                        (
                            "`use` associated items of traits is unstable".to_string(),
                            "E0658",
                        )
                    } else {
                        (format!("unresolved import `{}`", path.join("::")), "E0432")
                    }
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

}

impl<'a, 'b> RefVisitor<'a, 'b> {
    /// `name` in scope is only a fn, const or static: it cannot start a path (`func::x`).
    /// Off where a macro call or a glob that is not local could define a module of that name.
    fn value_item_as_path_root(&self, name: &str) -> bool {
        let v = &*self.validator;
        // A crate name is in the type namespace: a fn of the same name does not hide it.
        if v.symbols.has_macro_call(&v.module_path) || v.is_known_crate(name) {
            return false;
        }
        if !(v.scope_globs().is_empty() || self.unknown_behind_local_globs(name)) {
            return false;
        }
        if v.lookup_in_scope(name, Namespace::Type).is_some() {
            return false;
        }
        matches!(
            v.lookup_in_scope(name, Namespace::Value).map(|item| &item.kind),
            Some(ItemKind::Function | ItemKind::Const | ItemKind::Static)
        )
    }

    /// E0433 for an expression or type path whose module part names nothing in the crate.
    pub(super) fn check_path_modules(&mut self, path: &syn::Path, namespace: Namespace) {
        let names: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
        if names.len() < 2 {
            return;
        }
        if !self.validator.is_local_path(&names) {
            let first = names[0].as_str();
            if self.value_item_as_path_root(first) {
                self.validator.push_unknown_root(&names, path.segments[0].ident.span());
                return;
            }
            let defined_elsewhere = !self.validator.find_anywhere(first).is_empty();
            if !matches!(first, "crate" | "self" | "super")
                && !defined_elsewhere
            {
                self.validator.report_unknown_crate(&names, path.segments[0].ident.span());
            }
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
            Some(PathStop::Unrooted | PathStop::External { .. }) => return,
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
            namespace,
            &self.validator.module_path,
            self.validator.crate_name,
        );
        if is_private_step(true, found) {
            self.report_private(path, (last as isize + skipped) as usize, &abs[last]);
        }
    }

}
