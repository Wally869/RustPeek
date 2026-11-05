use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::Serialize;

/// A module path like `crate::parser::utils`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModulePath(pub Vec<String>);

impl ModulePath {
    pub fn root() -> Self {
        ModulePath(vec!["crate".to_string()])
    }

    pub fn child(&self, name: &str) -> Self {
        let mut segments = self.0.clone();
        segments.push(name.to_string());
        ModulePath(segments)
    }

    pub fn parent(&self) -> Option<Self> {
        if self.0.len() <= 1 {
            return None;
        }
        let mut segments = self.0.clone();
        segments.pop();
        Some(ModulePath(segments))
    }

    pub fn display(&self) -> String {
        self.0.join("::")
    }

    pub fn last(&self) -> &str {
        self.0.last().map(|s| s.as_str()).unwrap_or("crate")
    }
}

impl std::fmt::Display for ModulePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}

/// Visibility of an item
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Vis {
    Public,
    PubCrate,
    PubSuper,
    Private,
}

impl Vis {
    pub fn from_syn(vis: &syn::Visibility) -> Self {
        match vis {
            syn::Visibility::Public(_) => Vis::Public,
            syn::Visibility::Restricted(r) => {
                let path = &r.path;
                if path.is_ident("crate") {
                    Vis::PubCrate
                } else if path.is_ident("super") {
                    Vis::PubSuper
                } else {
                    Vis::Private
                }
            }
            syn::Visibility::Inherited => Vis::Private,
        }
    }

    /// Can this item be accessed from `accessor_module` when it's defined in `defining_module`?
    pub fn accessible_from(&self, defining_module: &ModulePath, accessor_module: &ModulePath) -> bool {
        match self {
            Vis::Public => true,
            Vis::PubCrate => true,
            Vis::PubSuper => {
                if let Some(parent) = defining_module.parent() {
                    // Accessible from the parent module and its children
                    accessor_module.0.starts_with(&parent.0)
                } else {
                    false
                }
            }
            Vis::Private => accessor_module.0.starts_with(&defining_module.0),
        }
    }
}

/// Outcome of resolving a name from an accessor module.
#[derive(Debug, Clone, Copy)]
pub enum Resolved<'a> {
    Accessible(&'a ItemInfo),
    /// The item exists but the item or a re-export on the way to it is not visible to the accessor.
    Inaccessible(&'a ItemInfo),
    NotFound,
}

impl<'a> Resolved<'a> {
    fn of(item: &'a ItemInfo, accessible: bool) -> Self {
        if accessible { Resolved::Accessible(item) } else { Resolved::Inaccessible(item) }
    }

    /// The item when found, whether or not the accessor can see it.
    pub fn item(self) -> Option<&'a ItemInfo> {
        match self {
            Resolved::Accessible(i) | Resolved::Inaccessible(i) => Some(i),
            Resolved::NotFound => None,
        }
    }

    pub fn accessible(self) -> Option<&'a ItemInfo> {
        match self {
            Resolved::Accessible(i) => Some(i),
            Resolved::Inaccessible(_) | Resolved::NotFound => None,
        }
    }
}

/// Kind of item in the symbol table
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemKind {
    Struct,
    Enum,
    Trait,
    Function,
    TypeAlias,
    Const,
    Static,
    Macro,
    Module,
}

/// A field in a struct
#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: String,
    pub vis: Vis,
}

/// An enum variant
#[derive(Debug, Clone)]
pub struct VariantInfo {
    pub name: String,
    pub field_count: usize,
    pub is_named_fields: bool,
    pub fields: Vec<FieldInfo>,
}

/// An item in the symbol table
#[derive(Debug, Clone)]
pub struct ItemInfo {
    pub name: String,
    pub kind: ItemKind,
    pub vis: Vis,
    pub module: ModulePath,
    /// Fields for structs
    pub fields: Vec<FieldInfo>,
    /// Variants for enums
    pub variants: Vec<VariantInfo>,
    /// Parameter count for functions (None if not a function)
    pub param_count: Option<usize>,
}

/// A use statement
#[derive(Debug, Clone)]
pub struct UseInfo {
    /// The full path being imported (e.g., `crate::parser::Parser`)
    pub path: Vec<String>,
    /// The name it's imported as (could be renamed via `as`)
    pub alias: String,
    /// Whether this is a glob import (`use foo::*`)
    pub is_glob: bool,
    /// Visibility of the use statement (for re-exports via `pub use`)
    pub vis: Vis,
    /// The module declaring the use (an inline module for uses inside `mod x { }`)
    pub module: ModulePath,
}

/// An impl block
#[derive(Debug, Clone)]
pub struct ImplInfo {
    /// The type name this impl is for (just the ident, not resolved)
    pub type_name: String,
    /// Method names and their param counts (excluding self)
    pub methods: Vec<MethodInfo>,
}

/// A method in an impl block
#[derive(Debug, Clone)]
pub struct MethodInfo {
    pub name: String,
    pub vis: Vis,
    /// Parameter count excluding self
    pub param_count: usize,
    pub has_self: bool,
}

/// All indexed information for a single module
#[derive(Debug, Clone, Default)]
pub struct ModuleInfo {
    pub items: Vec<ItemInfo>,
    pub uses: Vec<UseInfo>,
    pub impls: Vec<ImplInfo>,
    pub file_path: PathBuf,
    /// mod declarations in this module (child module names)
    pub child_modules: Vec<String>,
}

/// The full crate symbol table
#[derive(Debug, Default)]
pub struct SymbolTable {
    pub modules: HashMap<ModulePath, ModuleInfo>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Map a use path to an absolute crate path, resolving `self`, any run of `super`,
    /// the crate's own name and child-module prefixes against `from`.
    pub fn resolve_use_path(
        &self,
        from: &ModulePath,
        path: &[String],
        crate_name: Option<&str>,
    ) -> Option<Vec<String>> {
        let first = path.first()?.as_str();
        if first == "crate" {
            return Some(path.to_vec());
        }
        if crate_name == Some(first) {
            let mut resolved = vec!["crate".to_string()];
            resolved.extend(path[1..].iter().cloned());
            return Some(resolved);
        }
        match first {
            "self" => {
                let mut resolved = from.0.clone();
                resolved.extend(path[1..].iter().cloned());
                Some(resolved)
            }
            "super" => {
                let mut base = from.clone();
                let mut consumed = 0;
                while path.get(consumed).map(String::as_str) == Some("super") {
                    base = base.parent()?;
                    consumed += 1;
                }
                let mut resolved = base.0;
                resolved.extend(path[consumed..].iter().cloned());
                Some(resolved)
            }
            _ => {
                let candidate = from.child(first);
                if self.modules.contains_key(&candidate) {
                    let mut resolved = candidate.0;
                    resolved.extend(path[1..].iter().cloned());
                    Some(resolved)
                } else {
                    None
                }
            }
        }
    }

    /// Find what `name` refers to inside `module`: an own item, a followed `use`
    /// (re-exports included) or a glob import. Modules are items of kind `Module`.
    /// `accessor` is the module the lookup is made from.
    pub fn resolve_item(
        &self,
        module: &ModulePath,
        name: &str,
        accessor: &ModulePath,
        crate_name: Option<&str>,
    ) -> Resolved<'_> {
        self.resolve_item_in(module, name, accessor, crate_name, &mut HashSet::new())
    }

    /// Resolve a module reached by an absolute path, going through re-exported modules.
    pub fn resolve_module(
        &self,
        abs: &[String],
        accessor: &ModulePath,
        crate_name: Option<&str>,
    ) -> Option<ModulePath> {
        self.resolve_module_in(abs, accessor, crate_name, &mut HashSet::new())
    }

    fn resolve_item_in(
        &self,
        module: &ModulePath,
        name: &str,
        accessor: &ModulePath,
        crate_name: Option<&str>,
        in_progress: &mut HashSet<(ModulePath, String)>,
    ) -> Resolved<'_> {
        let key = (module.clone(), name.to_string());
        if !in_progress.insert(key.clone()) {
            return Resolved::NotFound;
        }
        let found = self.search_module(module, name, accessor, crate_name, in_progress);
        in_progress.remove(&key);
        found
    }

    fn search_module(
        &self,
        module: &ModulePath,
        name: &str,
        accessor: &ModulePath,
        crate_name: Option<&str>,
        in_progress: &mut HashSet<(ModulePath, String)>,
    ) -> Resolved<'_> {
        // An inline `mod` has no entry of its own: its items live in the enclosing file's
        // module, tagged with the inline module path.
        let Some(info) = self.modules.get(module).or_else(|| self.enclosing_file_module(module))
        else {
            return Resolved::NotFound;
        };
        if let Some(item) = info.items.iter().find(|i| i.name == name && i.module == *module) {
            return Resolved::of(item, item.vis.accessible_from(&item.module, accessor));
        }

        let mut inaccessible = None;
        for use_info in info.uses.iter().filter(|u| u.module == *module) {
            if use_info.is_glob || use_info.alias != name {
                continue;
            }
            let Some(abs) = self.resolve_use_path(module, &use_info.path, crate_name) else {
                continue;
            };
            let reachable = use_info.vis.accessible_from(module, accessor);
            match self.resolve_abs_item(&abs, accessor, crate_name, in_progress) {
                Resolved::Accessible(item) if reachable => return Resolved::Accessible(item),
                Resolved::Accessible(item) | Resolved::Inaccessible(item) => {
                    inaccessible.get_or_insert(item);
                }
                Resolved::NotFound => {}
            }
        }

        for use_info in info.uses.iter().filter(|u| u.module == *module) {
            if !use_info.is_glob || !use_info.vis.accessible_from(module, accessor) {
                continue;
            }
            let Some(abs) = self.resolve_use_path(module, &use_info.path, crate_name) else {
                continue;
            };
            let Some(target) = self.resolve_module_in(&abs, accessor, crate_name, in_progress)
            else {
                continue;
            };
            if let Resolved::Accessible(item) =
                self.resolve_item_in(&target, name, accessor, crate_name, in_progress)
            {
                return Resolved::Accessible(item);
            }
        }
        match inaccessible {
            Some(item) => Resolved::Inaccessible(item),
            None => Resolved::NotFound,
        }
    }

    fn enclosing_file_module(&self, module: &ModulePath) -> Option<&ModuleInfo> {
        let mut ancestor = module.parent();
        while let Some(path) = ancestor {
            if let Some(info) = self.modules.get(&path) {
                return Some(info);
            }
            ancestor = path.parent();
        }
        None
    }

    /// Does `module` exist: an indexed file module, or an inline module declared in its parent.
    pub fn module_exists(&self, module: &ModulePath) -> bool {
        if self.modules.contains_key(module) {
            return true;
        }
        let (Some(parent), Some(name)) = (module.parent(), module.0.last()) else {
            return false;
        };
        self.enclosing_file_module(module).is_some_and(|info| {
            info.items
                .iter()
                .any(|i| i.kind == ItemKind::Module && i.module == parent && i.name == *name)
        })
    }

    fn resolve_abs_item(
        &self,
        abs: &[String],
        accessor: &ModulePath,
        crate_name: Option<&str>,
        in_progress: &mut HashSet<(ModulePath, String)>,
    ) -> Resolved<'_> {
        let abs = match abs.split_last() {
            Some((last, rest)) if last == "self" && !rest.is_empty() => rest,
            _ => abs,
        };
        let Some((name, module_part)) = abs.split_last() else {
            return Resolved::NotFound;
        };
        let Some(module) = self.resolve_module_in(module_part, accessor, crate_name, in_progress)
        else {
            return Resolved::NotFound;
        };
        self.resolve_item_in(&module, name, accessor, crate_name, in_progress)
    }

    fn resolve_module_in(
        &self,
        abs: &[String],
        accessor: &ModulePath,
        crate_name: Option<&str>,
        in_progress: &mut HashSet<(ModulePath, String)>,
    ) -> Option<ModulePath> {
        if abs.first().map(String::as_str) != Some("crate") {
            return None;
        }
        let mut current = ModulePath::root();
        for segment in &abs[1..] {
            let item = self
                .resolve_item_in(&current, segment, accessor, crate_name, in_progress)
                .item()?;
            if item.kind != ItemKind::Module {
                return None;
            }
            current = item.module.child(&item.name);
        }
        Some(current)
    }

    /// Find impl methods for a type name
    pub fn find_methods(&self, type_name: &str) -> Vec<(&ModulePath, &MethodInfo)> {
        let mut results = Vec::new();
        for (path, module_info) in &self.modules {
            for impl_info in &module_info.impls {
                if impl_info.type_name == type_name {
                    for method in &impl_info.methods {
                        results.push((path, method));
                    }
                }
            }
        }
        results
    }
}

/// Severity of a diagnostic
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Suggestion,
}

/// A proposed auto-fix for a diagnostic
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum Fix {
    /// Insert a line at the given position
    InsertLine {
        file: PathBuf,
        /// Line number to insert BEFORE (1-indexed). 0 means append to end.
        line: usize,
        content: String,
    },
    /// Replace text on a specific line
    ReplaceLine {
        file: PathBuf,
        line: usize,
        old_text: String,
        new_text: String,
    },
    /// Remove a line entirely
    RemoveLine {
        file: PathBuf,
        line: usize,
    },
}

impl std::fmt::Display for Fix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fix::InsertLine { file, line, content } => {
                write!(f, "insert at {}:{}: {}", file.display(), line, content.trim())
            }
            Fix::ReplaceLine { file, line, old_text, new_text } => {
                write!(f, "replace at {}:{}: `{}` → `{}`", file.display(), line, old_text.trim(), new_text.trim())
            }
            Fix::RemoveLine { file, line } => {
                write!(f, "remove {}:{}", file.display(), line)
            }
        }
    }
}

/// A diagnostic produced by rustpeek
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub error_code: Option<String>,
    pub hint: Option<String>,
    /// Optional auto-fix for this diagnostic
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<Fix>,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let severity = match self.severity {
            Severity::Error => "error",
            Severity::Suggestion => "suggestion",
        };
        let code = self.error_code.as_deref().unwrap_or("");
        let code_part = if code.is_empty() {
            String::new()
        } else {
            format!("[{code}] ")
        };
        write!(
            f,
            "{severity}: {code_part}{msg}\n --> {file}:{line}:{col}",
            msg = self.message,
            file = self.file.display(),
            line = self.line,
            col = self.column,
        )?;
        if let Some(hint) = &self.hint {
            write!(f, "\n   = hint: {hint}")?;
        }
        Ok(())
    }
}

/// Result of running rustpeek analysis
#[derive(Debug, Serialize)]
pub struct AnalysisResult {
    pub diagnostics: Vec<Diagnostic>,
}

impl AnalysisResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Error).count()
    }

    pub fn suggestion_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Suggestion)
            .count()
    }

    pub fn fixable_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.fix.is_some()).count()
    }

    pub fn fixes(&self) -> Vec<&Fix> {
        self.diagnostics.iter().filter_map(|d| d.fix.as_ref()).collect()
    }
}
