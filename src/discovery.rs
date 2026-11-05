use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::types::ModulePath;

/// A module's source file and the directory its `mod x;` declarations are resolved against.
#[derive(Debug, Clone)]
pub struct ModuleFile {
    pub path: PathBuf,
    pub owned_dir: PathBuf,
}

/// The crate's module files, found by following `mod` declarations from the crate root.
#[derive(Debug)]
pub struct CrateFiles {
    pub files: HashMap<ModulePath, ModuleFile>,
}

pub fn discover_crate(root: &Path) -> CrateFiles {
    let src_dir = root.join("src");
    let mut files = HashMap::new();

    let Some(crate_root) = ["lib.rs", "main.rs"]
        .iter()
        .map(|name| src_dir.join(name))
        .find(|path| path.exists())
    else {
        return CrateFiles { files };
    };

    let root_file = ModuleFile { path: crate_root, owned_dir: src_dir };
    follow_declarations(&root_file, ModulePath::root(), &mut files, &mut Vec::new());
    files.insert(ModulePath::root(), root_file);
    CrateFiles { files }
}

fn follow_declarations(
    file: &ModuleFile,
    module_path: ModulePath,
    files: &mut HashMap<ModulePath, ModuleFile>,
    ancestors: &mut Vec<PathBuf>,
) {
    let Ok(source) = std::fs::read_to_string(&file.path) else {
        return;
    };
    // A syntax error is reported by the syntax pass on this file.
    let Some(ast) = crate::parser::parse_file(&source) else {
        return;
    };
    ancestors.push(file.path.clone());
    follow_items(&ast.items, file, &file.owned_dir, false, &module_path, files, ancestors);
    ancestors.pop();
}

fn follow_items(
    items: &[syn::Item],
    file: &ModuleFile,
    block_dir: &Path,
    inside_inline_mod: bool,
    module_path: &ModulePath,
    files: &mut HashMap<ModulePath, ModuleFile>,
    ancestors: &mut Vec<PathBuf>,
) {
    for item in items {
        let syn::Item::Mod(m) = item else {
            continue;
        };
        let child_path = module_path.child(&m.ident.to_string());
        if let Some((_, inner)) = &m.content {
            let inner_dir = block_dir.join(inline_dir_name(m));
            follow_items(inner, file, &inner_dir, true, &child_path, files, ancestors);
            continue;
        }
        let Some(child) = resolve_mod_file(&file.path, block_dir, inside_inline_mod, m) else {
            continue;
        };
        if files.contains_key(&child_path) || ancestors.contains(&child.path) {
            continue;
        }
        files.insert(child_path.clone(), child.clone());
        follow_declarations(&child, child_path, files, ancestors);
    }
}

/// The directory name an inline `mod x { }` adds to its children's lookup directory.
fn inline_dir_name(m: &syn::ItemMod) -> PathBuf {
    path_attribute(m).map_or_else(|| PathBuf::from(m.ident.to_string()), PathBuf::from)
}

fn path_attribute(m: &syn::ItemMod) -> Option<String> {
    m.attrs.iter().find_map(|attr| {
        if !attr.path().is_ident("path") {
            return None;
        }
        match &attr.meta.require_name_value().ok()?.value {
            syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) => Some(s.value()),
            _ => None,
        }
    })
}

/// Map a `mod x;` declaration to its file, following rustc: `#[path]` is relative to the
/// declaring file's directory at the top level of the file, and to `block_dir` inside an
/// inline module. Without it the file is `{block_dir}/x.rs` or `{block_dir}/x/mod.rs`.
/// `block_dir` starts as the declaring module's owned directory.
pub(crate) fn resolve_mod_file(
    declaring_file: &Path,
    block_dir: &Path,
    inside_inline_mod: bool,
    m: &syn::ItemMod,
) -> Option<ModuleFile> {
    if let Some(attr_path) = path_attribute(m) {
        let base = if inside_inline_mod { block_dir } else { declaring_file.parent()? };
        let path = base.join(attr_path);
        let owned_dir = path.parent()?.to_path_buf();
        return path.is_file().then_some(ModuleFile { path, owned_dir });
    }
    let name = m.ident.to_string();
    let flat = block_dir.join(format!("{name}.rs"));
    if flat.is_file() {
        return Some(ModuleFile { path: flat, owned_dir: block_dir.join(&name) });
    }
    let nested = block_dir.join(&name).join("mod.rs");
    nested.is_file().then(|| ModuleFile { owned_dir: block_dir.join(&name), path: nested })
}
