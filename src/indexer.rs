use std::path::Path;

use crate::types::*;

/// Index a single parsed file and produce its ModuleInfo.
pub fn index_file(ast: &syn::File, module_path: &ModulePath, file_path: &Path) -> ModuleInfo {
    let mut info = ModuleInfo {
        file_path: file_path.to_path_buf(),
        ..Default::default()
    };

    for item in &ast.items {
        index_item(item, module_path, &mut info);
    }

    info
}

fn index_item(item: &syn::Item, module_path: &ModulePath, info: &mut ModuleInfo) {
    match item {
        syn::Item::Struct(s) => {
            let fields = match &s.fields {
                syn::Fields::Named(named) => named
                    .named
                    .iter()
                    .filter_map(|f| {
                        f.ident.as_ref().map(|name| FieldInfo {
                            name: name.to_string(),
                            vis: Vis::from_syn(&f.vis),
                        })
                    })
                    .collect(),
                _ => Vec::new(),
            };

            info.items.push(ItemInfo {
                name: s.ident.to_string(),
                kind: ItemKind::Struct,
                vis: Vis::from_syn(&s.vis),
                module: module_path.clone(),
                fields,
                variants: Vec::new(),
                param_count: None,
            });
        }

        syn::Item::Enum(e) => {
            let variants = e
                .variants
                .iter()
                .map(|v| {
                    let (field_count, is_named, fields) = match &v.fields {
                        syn::Fields::Named(named) => (
                            named.named.len(),
                            true,
                            named
                                .named
                                .iter()
                                .filter_map(|f| {
                                    f.ident.as_ref().map(|name| FieldInfo {
                                        name: name.to_string(),
                                        vis: Vis::from_syn(&f.vis),
                                    })
                                })
                                .collect(),
                        ),
                        syn::Fields::Unnamed(unnamed) => {
                            (unnamed.unnamed.len(), false, Vec::new())
                        }
                        syn::Fields::Unit => (0, false, Vec::new()),
                    };
                    VariantInfo {
                        name: v.ident.to_string(),
                        field_count,
                        is_named_fields: is_named,
                        fields,
                    }
                })
                .collect();

            info.items.push(ItemInfo {
                name: e.ident.to_string(),
                kind: ItemKind::Enum,
                vis: Vis::from_syn(&e.vis),
                module: module_path.clone(),
                fields: Vec::new(),
                variants,
                param_count: None,
            });
        }

        syn::Item::Trait(t) => {
            info.items.push(ItemInfo {
                name: t.ident.to_string(),
                kind: ItemKind::Trait,
                vis: Vis::from_syn(&t.vis),
                module: module_path.clone(),
                fields: Vec::new(),
                variants: Vec::new(),
                param_count: None,
            });
        }

        syn::Item::Fn(f) => {
            let param_count = count_fn_params(&f.sig);
            info.items.push(ItemInfo {
                name: f.sig.ident.to_string(),
                kind: ItemKind::Function,
                vis: Vis::from_syn(&f.vis),
                module: module_path.clone(),
                fields: Vec::new(),
                variants: Vec::new(),
                param_count: Some(param_count),
            });
        }

        syn::Item::Type(t) => {
            info.items.push(ItemInfo {
                name: t.ident.to_string(),
                kind: ItemKind::TypeAlias,
                vis: Vis::from_syn(&t.vis),
                module: module_path.clone(),
                fields: Vec::new(),
                variants: Vec::new(),
                param_count: None,
            });
        }

        syn::Item::Const(c) => {
            info.items.push(ItemInfo {
                name: c.ident.to_string(),
                kind: ItemKind::Const,
                vis: Vis::from_syn(&c.vis),
                module: module_path.clone(),
                fields: Vec::new(),
                variants: Vec::new(),
                param_count: None,
            });
        }

        syn::Item::Static(s) => {
            info.items.push(ItemInfo {
                name: s.ident.to_string(),
                kind: ItemKind::Static,
                vis: Vis::from_syn(&s.vis),
                module: module_path.clone(),
                fields: Vec::new(),
                variants: Vec::new(),
                param_count: None,
            });
        }

        syn::Item::Mod(m) => {
            let mod_name = m.ident.to_string();
            info.child_modules.push(mod_name.clone());

            // If the module has inline content, index it
            if let Some((_, items)) = &m.content {
                let child_path = module_path.child(&mod_name);
                info.items.push(ItemInfo {
                    name: mod_name.clone(),
                    kind: ItemKind::Module,
                    vis: Vis::from_syn(&m.vis),
                    module: module_path.clone(),
                    fields: Vec::new(),
                    variants: Vec::new(),
                    param_count: None,
                });

                // Recursively index inline module items
                let mut child_info = ModuleInfo {
                    file_path: info.file_path.clone(),
                    ..Default::default()
                };
                for item in items {
                    index_item(item, &child_path, &mut child_info);
                }
                // Merge child info back — the caller will handle storing this
                // For now, we store inline module items in the parent
                info.items.extend(child_info.items);
                info.uses.extend(child_info.uses);
                info.impls.extend(child_info.impls);
            } else {
                // External module declaration
                info.items.push(ItemInfo {
                    name: mod_name,
                    kind: ItemKind::Module,
                    vis: Vis::from_syn(&m.vis),
                    module: module_path.clone(),
                    fields: Vec::new(),
                    variants: Vec::new(),
                    param_count: None,
                });
            }
        }

        syn::Item::Use(u) => {
            collect_use_tree(&u.tree, &mut Vec::new(), info);
        }

        syn::Item::Impl(imp) => {
            if imp.trait_.is_some() {
                // Trait impl — we only index direct impls for now
                return;
            }

            let type_name = extract_type_name(&imp.self_ty);
            if let Some(type_name) = type_name {
                let methods: Vec<MethodInfo> = imp
                    .items
                    .iter()
                    .filter_map(|item| {
                        if let syn::ImplItem::Fn(method) = item {
                            let has_self = method
                                .sig
                                .inputs
                                .first()
                                .is_some_and(|arg| matches!(arg, syn::FnArg::Receiver(_)));
                            let param_count = count_fn_params(&method.sig);
                            Some(MethodInfo {
                                name: method.sig.ident.to_string(),
                                vis: Vis::from_syn(&method.vis),
                                param_count,
                                has_self,
                            })
                        } else {
                            None
                        }
                    })
                    .collect();

                info.impls.push(ImplInfo {
                    type_name,
                    methods,
                });
            }
        }

        syn::Item::Macro(m) => {
            if let Some(ident) = &m.ident {
                info.items.push(ItemInfo {
                    name: ident.to_string(),
                    kind: ItemKind::Macro,
                    vis: Vis::Private, // macro_rules! visibility is complex
                    module: module_path.clone(),
                    fields: Vec::new(),
                    variants: Vec::new(),
                    param_count: None,
                });
            }
        }

        _ => {}
    }
}

/// Count function parameters, excluding `self`.
fn count_fn_params(sig: &syn::Signature) -> usize {
    sig.inputs
        .iter()
        .filter(|arg| matches!(arg, syn::FnArg::Typed(_)))
        .count()
}

/// Extract the type name from a type expression (just the ident).
fn extract_type_name(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(p) => p.path.segments.last().map(|s| s.ident.to_string()),
        syn::Type::Reference(r) => extract_type_name(&r.elem),
        _ => None,
    }
}

/// Recursively collect use statements from a use tree.
fn collect_use_tree(tree: &syn::UseTree, prefix: &mut Vec<String>, info: &mut ModuleInfo) {
    match tree {
        syn::UseTree::Path(p) => {
            prefix.push(p.ident.to_string());
            collect_use_tree(&p.tree, prefix, info);
            prefix.pop();
        }
        syn::UseTree::Name(n) => {
            let mut path = prefix.clone();
            path.push(n.ident.to_string());
            let alias = n.ident.to_string();
            info.uses.push(UseInfo {
                path,
                alias,
                is_glob: false,
            });
        }
        syn::UseTree::Rename(r) => {
            let mut path = prefix.clone();
            path.push(r.ident.to_string());
            let alias = r.rename.to_string();
            info.uses.push(UseInfo {
                path,
                alias,
                is_glob: false,
            });
        }
        syn::UseTree::Glob(_) => {
            info.uses.push(UseInfo {
                path: prefix.clone(),
                alias: String::new(),
                is_glob: true,
            });
        }
        syn::UseTree::Group(g) => {
            for tree in &g.items {
                collect_use_tree(tree, prefix, info);
            }
        }
    }
}
