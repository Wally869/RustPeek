use std::path::Path;

use syn::visit::Visit;

use crate::discovery;

use crate::types::*;

struct VisPathCheck<'a> {
    diagnostics: &'a mut Vec<Diagnostic>,
    file_path: &'a Path,
    module_path: ModulePath,
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
    /// The directory `mod x;` declarations of the file are resolved against.
    owned_dir: &'a Path,
    /// Lazily loaded source lines for fix generation
    source_lines: Option<Vec<String>>,
    /// The crate's own name (from Cargo.toml), so `use <name>::...` is treated as `use crate::...`
    crate_name: Option<&'a str>,
    local_scopes: Vec<LocalScope>,
}

/// Visitor that walks expressions looking for references to validate.
struct RefVisitor<'a, 'b> {
    validator: &'a mut ValidationVisitor<'b>,
}

#[derive(Default)]
struct LocalScope {
    imports: Vec<LocalImport>,
    globs: Vec<Vec<String>>,
    /// Names bound without a path to resolve: generic parameters, items declared in a block.
    names: Vec<String>,
    /// The block holds an item-like macro invocation (`m! { .. }`), which may define any item.
    has_macro_items: bool,
}

/// Names the std prelude and the language bind in every module.
const PRELUDE_NAMES: &[&str] = &[
    "Self", "Option", "Some", "None", "Result", "Ok", "Err", "Vec", "String", "Box",
    "Copy", "Send", "Sized", "Sync", "Unpin", "Drop", "Fn", "FnMut", "FnOnce",
    "Clone", "Eq", "PartialEq", "Ord", "PartialOrd", "Default", "AsRef", "AsMut", "Into",
    "From", "ToOwned", "ToString", "TryFrom", "TryInto", "FromIterator", "Iterator",
    "IntoIterator", "DoubleEndedIterator", "ExactSizeIterator", "Extend",
    "bool", "char", "str", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize",
    "u8", "u16", "u32", "u64", "u128", "usize",
    "drop", "size_of", "size_of_val", "align_of", "align_of_val", "Future", "IntoFuture",
];

struct LocalImport {
    name: String,
    path: Vec<String>,
}

enum LocalLookup<'a> {
    Item(&'a ItemInfo),
    Imported,
    None,
}

/// Validate references in a file's AST against the symbol table.
pub fn validate_file(
    ast: &syn::File,
    file_path: &Path,
    module_path: &ModulePath,
    symbols: &SymbolTable,
    owned_dir: &Path,
    crate_name: Option<&str>,
) -> Vec<Diagnostic> {
    let mut visitor = ValidationVisitor {
        diagnostics: Vec::new(),
        file_path,
        module_path: module_path.clone(),
        symbols,
        owned_dir,
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

mod names;
mod validate;
mod local;
mod scope;
mod check;
mod ast;

use names::pattern_names;
use names::signature_names;
use names::levenshtein;
use validate::is_similar;
use local::is_private_step;
use local::collect_local_imports;
use local::path_last_segment;
use scope::declared_item_name;
