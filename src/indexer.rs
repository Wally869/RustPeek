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
    merge_cfg_alternatives(&mut info.items);

    info
}

fn item_attrs(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Struct(i) => &i.attrs,
        syn::Item::Enum(i) => &i.attrs,
        syn::Item::Trait(i) => &i.attrs,
        syn::Item::Fn(i) => &i.attrs,
        syn::Item::Type(i) => &i.attrs,
        syn::Item::Const(i) => &i.attrs,
        syn::Item::Static(i) => &i.attrs,
        syn::Item::Mod(i) => &i.attrs,
        syn::Item::Macro(i) => &i.attrs,
        _ => &[],
    }
}

fn index_item(item: &syn::Item, module_path: &ModulePath, info: &mut ModuleInfo) {
    let first_new = info.items.len();
    index_item_body(item, module_path, info);
    let cfg = cfg_conditions(item_attrs(item));
    if cfg.is_empty() {
        return;
    }
    for indexed in info.items[first_new..].iter_mut().filter(|i| i.module == *module_path) {
        indexed.cfg = cfg.clone();
    }
}

/// Items that are cfg alternatives of one another share what a check may rely on: every
/// parameter count of a function, and the fields of a struct, each optional unless every
/// alternative has it.
fn merge_cfg_alternatives(items: &mut [ItemInfo]) {
    for index in 0..items.len() {
        let group: Vec<usize> = (0..items.len())
            .filter(|&other| other == index || items[other].is_cfg_alternative_of(&items[index]))
            .collect();
        if group.len() < 2 {
            continue;
        }
        let alt_param_counts: Vec<usize> =
            group.iter().filter_map(|&g| items[g].param_count).collect();
        let mut fields: Vec<FieldInfo> = Vec::new();
        for &g in &group {
            for field in &items[g].fields {
                match fields.iter_mut().find(|f| f.name == field.name) {
                    Some(known) => known.cfg_gated |= field.cfg_gated,
                    None => fields.push(field.clone()),
                }
            }
        }
        for field in &mut fields {
            field.cfg_gated |= group.iter().any(|&g| !items[g].fields.iter().any(|f| f.name == field.name));
        }
        let group_kind_is_struct = items[index].kind == ItemKind::Struct;
        let target = &mut items[index];
        if target.param_count.is_some() {
            target.alt_param_counts = alt_param_counts;
        }
        if group_kind_is_struct {
            target.fields = fields;
        }
    }
}

fn index_item_body(item: &syn::Item, module_path: &ModulePath, info: &mut ModuleInfo) {
    match item {
        syn::Item::Struct(s) => {
            let fields = match &s.fields {
                syn::Fields::Named(named) => named
                    .named
                    .iter()
                    .filter_map(|f| {
                        f.ident.as_ref().map(|name| FieldInfo {
                            name: name.to_string(),
                            vis: Vis::from_syn(&f.vis, module_path),
                            cfg_gated: !cfg_conditions(&f.attrs).is_empty(),
                        })
                    })
                    .collect(),
                _ => Vec::new(),
            };

            index_derives(&s.attrs, &s.ident.to_string(), info);
            info.items.push(ItemInfo {
                name: s.ident.to_string(),
                kind: ItemKind::Struct,
                cfg: Vec::new(),
                alt_param_counts: Vec::new(),
                has_value_ctor: !matches!(s.fields, syn::Fields::Named(_)),
                has_macro_items: false,
                vis: Vis::from_syn(&s.vis, module_path),
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
                                        vis: Vis::from_syn(&f.vis, module_path),
                                        cfg_gated: false,
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

            index_derives(&e.attrs, &e.ident.to_string(), info);
            info.items.push(ItemInfo {
                name: e.ident.to_string(),
                kind: ItemKind::Enum,
                cfg: Vec::new(),
                alt_param_counts: Vec::new(),
                has_value_ctor: false,
                has_macro_items: false,
                vis: Vis::from_syn(&e.vis, module_path),
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
                cfg: Vec::new(),
                alt_param_counts: Vec::new(),
                has_value_ctor: false,
                has_macro_items: t
                    .items
                    .iter()
                    .any(|item| matches!(item, syn::TraitItem::Macro(_))),
                vis: Vis::from_syn(&t.vis, module_path),
                module: module_path.clone(),
                fields: trait_members(t, &Vis::from_syn(&t.vis, module_path)),
                variants: Vec::new(),
                param_count: None,
            });
        }

        syn::Item::Fn(f) => {
            let param_count = count_fn_params(&f.sig);
            info.items.push(ItemInfo {
                name: f.sig.ident.to_string(),
                kind: ItemKind::Function,
                cfg: Vec::new(),
                alt_param_counts: Vec::new(),
                has_value_ctor: false,
                has_macro_items: false,
                vis: Vis::from_syn(&f.vis, module_path),
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
                cfg: Vec::new(),
                alt_param_counts: Vec::new(),
                has_value_ctor: false,
                has_macro_items: false,
                vis: Vis::from_syn(&t.vis, module_path),
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
                cfg: Vec::new(),
                alt_param_counts: Vec::new(),
                has_value_ctor: false,
                has_macro_items: false,
                vis: Vis::from_syn(&c.vis, module_path),
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
                cfg: Vec::new(),
                alt_param_counts: Vec::new(),
                has_value_ctor: false,
                has_macro_items: false,
                vis: Vis::from_syn(&s.vis, module_path),
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
                    cfg: Vec::new(),
                    alt_param_counts: Vec::new(),
                    has_value_ctor: false,
                    has_macro_items: false,
                    vis: Vis::from_syn(&m.vis, module_path),
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
                info.macro_modules.extend(child_info.macro_modules);
            } else {
                // External module declaration
                info.items.push(ItemInfo {
                    name: mod_name,
                    kind: ItemKind::Module,
                    cfg: Vec::new(),
                    alt_param_counts: Vec::new(),
                    has_value_ctor: false,
                    has_macro_items: false,
                    vis: Vis::from_syn(&m.vis, module_path),
                    module: module_path.clone(),
                    fields: Vec::new(),
                    variants: Vec::new(),
                    param_count: None,
                });
            }
        }

        syn::Item::Use(u) => {
            let vis = Vis::from_syn(&u.vis, module_path);
            collect_use_tree(&u.tree, &mut Vec::new(), info, &vis, module_path);
        }

        // Recorded as a named use of the crate's own name: an external path binding `name`.
        syn::Item::ExternCrate(e) => {
            let alias = e.rename.as_ref().map_or(&e.ident, |(_, alias)| alias).to_string();
            info.uses.push(UseInfo {
                path: vec![e.ident.to_string()],
                alias,
                is_glob: false,
                vis: Vis::from_syn(&e.vis, module_path),
                module: module_path.clone(),
            });
        }

        syn::Item::Impl(imp) => {
            let type_name = extract_type_name(&imp.self_ty);
            if let Some(type_name) = type_name {
                let trait_name = imp
                    .trait_
                    .as_ref()
                    .and_then(|(_, path, _)| path.segments.last().map(|s| s.ident.to_string()));
                let is_blanket = trait_name.is_some()
                    && imp.generics.type_params().any(|p| p.ident == type_name.as_str());
                let has_macro = imp.items.iter().any(|item| matches!(item, syn::ImplItem::Macro(_)));
                let assoc_names: Vec<String> = imp
                    .items
                    .iter()
                    .filter_map(|item| match item {
                        syn::ImplItem::Const(c) => Some(c.ident.to_string()),
                        syn::ImplItem::Type(t) => Some(t.ident.to_string()),
                        _ => None,
                    })
                    .collect();
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
                                vis: Vis::from_syn(&method.vis, module_path),
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
                    assoc_names,
                    trait_name,
                    is_derive: false,
                    is_blanket,
                    has_macro,
                });
            }
        }

        syn::Item::Macro(m) => {
            if m.ident.is_none() {
                // An invocation the tool cannot expand: it may define any item.
                info.macro_modules.push(module_path.clone());
            }
            if let Some(ident) = &m.ident {
                let exported = m.attrs.iter().any(|a| a.path().is_ident("macro_export"));
                info.items.push(ItemInfo {
                    name: ident.to_string(),
                    kind: ItemKind::Macro,
                    cfg: Vec::new(),
                    alt_param_counts: Vec::new(),
                has_value_ctor: false,
                has_macro_items: false,
                    vis: if exported {
                        Vis::Public
                    } else {
                        Vis::Restricted(module_path.clone())
                    },
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

/// One derive entry per name of the `#[derive(..)]` attributes of a type. An attribute the
/// tool cannot read (`cfg_attr`, a malformed derive) is recorded as an unknown derive.
fn index_derives(attrs: &[syn::Attribute], type_name: &str, info: &mut ModuleInfo) {
    let mut names = Vec::new();
    for attr in attrs {
        if attr.path().is_ident("derive") {
            let parsed = attr.parse_nested_meta(|meta| {
                if let Some(last) = meta.path.segments.last() {
                    names.push(last.ident.to_string());
                }
                Ok(())
            });
            if parsed.is_err() {
                names.push("unreadable".to_string());
            }
        } else if attr.path().is_ident("cfg_attr") {
            names.push("cfg_attr".to_string());
        }
    }
    for name in names {
        info.impls.push(ImplInfo {
            type_name: type_name.to_string(),
            methods: Vec::new(),
            assoc_names: Vec::new(),
            trait_name: Some(name),
            is_derive: true,
            is_blanket: false,
            has_macro: false,
        });
    }
}

/// The associated items of a trait (fns, consts, types), with the trait's visibility.
fn trait_members(t: &syn::ItemTrait, vis: &Vis) -> Vec<FieldInfo> {
    t.items
        .iter()
        .filter_map(|item| match item {
            syn::TraitItem::Fn(f) => Some(&f.sig.ident),
            syn::TraitItem::Const(c) => Some(&c.ident),
            syn::TraitItem::Type(ty) => Some(&ty.ident),
            _ => None,
        })
        .map(|ident| FieldInfo { name: ident.to_string(), vis: vis.clone(), cfg_gated: false })
        .collect()
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
fn collect_use_tree(tree: &syn::UseTree, prefix: &mut Vec<String>, info: &mut ModuleInfo, vis: &Vis, module: &ModulePath) {
    match tree {
        syn::UseTree::Path(p) => {
            prefix.push(p.ident.to_string());
            collect_use_tree(&p.tree, prefix, info, vis, module);
            prefix.pop();
        }
        syn::UseTree::Name(n) => {
            let mut path = prefix.clone();
            path.push(n.ident.to_string());
            // `use a::b::{self}` binds `b`; the path keeps its trailing `self`.
            let alias = match prefix.last() {
                Some(last) if n.ident == "self" => last.clone(),
                _ => n.ident.to_string(),
            };
            info.uses.push(UseInfo {
                path,
                alias,
                is_glob: false,
                vis: vis.clone(),
                module: module.clone(),
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
                vis: vis.clone(),
                module: module.clone(),
            });
        }
        syn::UseTree::Glob(_) => {
            info.uses.push(UseInfo {
                path: prefix.clone(),
                alias: String::new(),
                is_glob: true,
                vis: vis.clone(),
                module: module.clone(),
            });
        }
        syn::UseTree::Group(g) => {
            for tree in &g.items {
                collect_use_tree(tree, prefix, info, vis, module);
            }
        }
    }
}
