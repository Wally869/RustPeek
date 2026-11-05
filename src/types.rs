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
    /// Visible in this absolute module and its descendants.
    Restricted(ModulePath),
}

impl Vis {
    pub fn from_syn(vis: &syn::Visibility, defining_module: &ModulePath) -> Self {
        match vis {
            syn::Visibility::Public(_) => Vis::Public,
            syn::Visibility::Restricted(r) => {
                // An unresolvable path is reported as E0433 by the validator; the item stays usable.
                match resolve_vis_path(&r.path, defining_module) {
                    Some(module) => Vis::Restricted(module),
                    None => Vis::Public,
                }
            }
            syn::Visibility::Inherited => Vis::Restricted(defining_module.clone()),
        }
    }

    pub fn accessible_from(&self, accessor_module: &ModulePath) -> bool {
        match self {
            Vis::Public => true,
            Vis::Restricted(module) => accessor_module.0.starts_with(&module.0),
        }
    }
}

/// None when a `super` climbs past the crate root (rustc E0433).
pub fn resolve_vis_path(path: &syn::Path, defining_module: &ModulePath) -> Option<ModulePath> {
    let mut current = defining_module.clone();
    for (i, seg) in path.segments.iter().enumerate() {
        let name = seg.ident.to_string();
        match name.as_str() {
            "crate" if i == 0 => current = ModulePath::root(),
            "self" if i == 0 => {}
            "super" => current = current.parent()?,
            _ => current = current.child(&name),
        }
    }
    Some(current)
}

/// State of one resolution: the lookups in progress (cycle guard) and the visibility of each
/// `use` link on the chain that resolved the name, nearest the item first.
#[derive(Default)]
struct Walk {
    active: HashSet<(ModulePath, String)>,
    links: Vec<Vis>,
}

/// Outcome of resolving a name from an accessor module.
#[derive(Debug, Clone, Copy)]
pub enum Resolved<'a> {
    Accessible(&'a ItemInfo),
    /// The item exists but the item or a re-export on the way to it is not visible to the accessor.
    Inaccessible(&'a ItemInfo),
    NotFound,
    /// The name is bound outside the crate: an `extern crate`, a `use` of an external path,
    /// or a name an external glob may bring.
    External,
}

/// A module reached by a path. `accessible` is false when a module along the path is not
/// visible to the accessor.
#[derive(Debug, Clone)]
pub struct ResolvedModule {
    pub path: ModulePath,
    pub accessible: bool,
}

/// Why a walk along an absolute path stopped before its end.
#[derive(Debug, Clone, Copy)]
pub enum PathStop {
    /// The path does not start at `crate`.
    Unrooted,
    /// No item of that name at `index`.
    Missing { index: usize },
    /// The item at `index` is not a module.
    NotModule { index: usize },
    /// The name at `index` is bound outside the crate; the rest of the path is not ours to check.
    External { index: usize },
}

/// Outcome of walking an absolute path: the module reached, whether every module on the
/// way is visible, the first segment not visible, and where the walk stopped, if it did.
#[derive(Debug, Clone)]
pub struct PathWalk {
    pub reached: ModulePath,
    pub accessible: bool,
    pub first_private: Option<usize>,
    pub stop: Option<PathStop>,
}

impl PathWalk {
    /// The module the whole path names; `None` when the walk stopped.
    pub fn module(self) -> Option<ResolvedModule> {
        self.stop
            .is_none()
            .then(|| ResolvedModule { path: self.reached, accessible: self.accessible })
    }
}

impl<'a> Resolved<'a> {
    fn of(item: &'a ItemInfo, accessible: bool) -> Self {
        if accessible { Resolved::Accessible(item) } else { Resolved::Inaccessible(item) }
    }

    /// The item when found, whether or not the accessor can see it.
    pub fn item(self) -> Option<&'a ItemInfo> {
        match self {
            Resolved::Accessible(i) | Resolved::Inaccessible(i) => Some(i),
            Resolved::NotFound | Resolved::External => None,
        }
    }

    pub fn accessible(self) -> Option<&'a ItemInfo> {
        match self {
            Resolved::Accessible(i) => Some(i),
            Resolved::Inaccessible(_) | Resolved::NotFound | Resolved::External => None,
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
    /// Fields for structs; associated items (fns, consts, types) for traits
    pub fields: Vec<FieldInfo>,
    /// Variants for enums
    pub variants: Vec<VariantInfo>,
    /// Parameter count for functions (None if not a function)
    pub param_count: Option<usize>,
    /// Unit or tuple struct: also binds a constructor in the value namespace
    pub has_value_ctor: bool,
    /// Trait whose body holds an item-like macro call: it may define any member.
    pub has_macro_items: bool,
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
    /// This module and its inline modules that hold an item-level macro invocation
    pub macro_modules: Vec<ModulePath>,
}

/// The full crate symbol table
#[derive(Debug, Default)]
pub struct SymbolTable {
    pub modules: HashMap<ModulePath, ModuleInfo>,
    /// Dependencies and `extern crate` names; None when the dependencies are unknown.
    pub external_crates: Option<HashSet<String>>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// A crate name usable without a local declaration: a sysroot crate in the extern
    /// prelude (`alloc` and `test` need `extern crate`), a dependency or an `extern crate`.
    pub fn is_known_crate(&self, name: &str) -> bool {
        matches!(name, "std" | "core" | "proc_macro")
            || self.external_crates.as_ref().is_some_and(|c| c.contains(name))
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
                if self.module_exists(&candidate) {
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
        self.resolve_item_in(module, name, accessor, crate_name, &mut Walk::default())
    }

    /// Like `resolve_item`, also returning the visibility of every `use` link the name
    /// resolved through (empty when the item is defined in `module` itself).
    pub fn resolve_item_links(
        &self,
        module: &ModulePath,
        name: &str,
        accessor: &ModulePath,
        crate_name: Option<&str>,
    ) -> (Resolved<'_>, Vec<Vis>) {
        let mut walk = Walk::default();
        let found = self.resolve_item_in(module, name, accessor, crate_name, &mut walk);
        (found, walk.links)
    }

    fn resolve_item_in(
        &self,
        module: &ModulePath,
        name: &str,
        accessor: &ModulePath,
        crate_name: Option<&str>,
        in_progress: &mut Walk,
    ) -> Resolved<'_> {
        let key = (module.clone(), name.to_string());
        if !in_progress.active.insert(key.clone()) {
            return Resolved::NotFound;
        }
        let found = self.search_module(module, name, accessor, crate_name, in_progress);
        in_progress.active.remove(&key);
        found
    }

    fn search_module(
        &self,
        module: &ModulePath,
        name: &str,
        accessor: &ModulePath,
        crate_name: Option<&str>,
        in_progress: &mut Walk,
    ) -> Resolved<'_> {
        // An inline `mod` has no entry of its own: its items live in the enclosing file's
        // module, tagged with the inline module path.
        let Some(info) = self.modules.get(module).or_else(|| self.enclosing_file_module(module))
        else {
            return Resolved::NotFound;
        };
        if let Some(item) = info.items.iter().find(|i| i.name == name && i.module == *module) {
            return Resolved::of(item, item.vis.accessible_from(accessor));
        }

        let mut inaccessible = None;
        let mut external = false;
        for use_info in info.uses.iter().filter(|u| u.module == *module) {
            if use_info.is_glob || use_info.alias != name {
                continue;
            }
            let Some(abs) = self.resolve_use_path(module, &use_info.path, crate_name) else {
                external = true;
                continue;
            };
            let reachable = use_info.vis.accessible_from(accessor);
            let mark = in_progress.links.len();
            match self.resolve_abs_item(&abs, module, accessor, crate_name, in_progress) {
                Resolved::Accessible(item) if reachable => {
                    in_progress.links.push(use_info.vis.clone());
                    return Resolved::Accessible(item);
                }
                Resolved::Accessible(item) | Resolved::Inaccessible(item) => {
                    inaccessible.get_or_insert(item);
                }
                Resolved::External => external = true,
                Resolved::NotFound => {}
            }
            in_progress.links.truncate(mark);
        }

        for use_info in info.uses.iter().filter(|u| u.module == *module) {
            if !use_info.is_glob {
                continue;
            }
            let Some(abs) = self.resolve_use_path(module, &use_info.path, crate_name) else {
                // A glob of an unknown crate brings nothing.
                let may_bring = self.external_crates.is_none()
                    || use_info.path.first().is_some_and(|first| self.is_known_crate(first));
                external |= may_bring;
                continue;
            };
            let glob_walk = self.walk_path_in(&abs, module, crate_name, in_progress);
            if matches!(glob_walk.stop, Some(PathStop::External { .. })) {
                external = true;
                continue;
            }
            let Some(target) = glob_walk.module() else {
                continue;
            };
            // A glob imports only what its holder can see; the rest is skipped but still
            // exists behind the glob, so an explicit path to it is private, not missing.
            let mark = in_progress.links.len();
            let seen_by_holder =
                self.resolve_item_in(&target.path, name, module, crate_name, in_progress);
            in_progress.links.truncate(mark);
            match seen_by_holder {
                Resolved::Accessible(_) => {
                    match self.resolve_item_in(&target.path, name, accessor, crate_name, in_progress) {
                        Resolved::Accessible(item)
                            if target.accessible
                                && use_info.vis.accessible_from(accessor) =>
                        {
                            in_progress.links.push(use_info.vis.clone());
                            return Resolved::Accessible(item);
                        }
                        Resolved::Accessible(item) | Resolved::Inaccessible(item) => {
                            inaccessible.get_or_insert(item);
                        }
                        Resolved::External => external = true,
                        Resolved::NotFound => {}
                    }
                    in_progress.links.truncate(mark);
                }
                Resolved::Inaccessible(item) => {
                    inaccessible.get_or_insert(item);
                }
                Resolved::External => external = true,
                Resolved::NotFound => {}
            }
        }
        // An item-level macro call in the module may define the name: not ours to check.
        match inaccessible {
            Some(item) => Resolved::Inaccessible(item),
            None if external || self.has_macro_call(module) => Resolved::External,
            None => Resolved::NotFound,
        }
    }

    /// Paths of the glob imports declared directly in `module`.
    pub(crate) fn glob_paths(&self, module: &ModulePath) -> Vec<&[String]> {
        let Some(info) = self.modules.get(module).or_else(|| self.enclosing_file_module(module))
        else {
            return Vec::new();
        };
        info.uses
            .iter()
            .filter(|u| u.is_glob && u.module == *module)
            .map(|u| u.path.as_slice())
            .collect()
    }

    pub(crate) fn enclosing_file_module(&self, module: &ModulePath) -> Option<&ModuleInfo> {
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

    /// `path_accessor` is the module the path is written in (it sees the modules along the path);
    /// `accessor` is the module the final lookup is made from.
    fn resolve_abs_item(
        &self,
        abs: &[String],
        path_accessor: &ModulePath,
        accessor: &ModulePath,
        crate_name: Option<&str>,
        in_progress: &mut Walk,
    ) -> Resolved<'_> {
        let abs = match abs.split_last() {
            Some((last, rest)) if last == "self" && !rest.is_empty() => rest,
            _ => abs,
        };
        let Some((name, module_part)) = abs.split_last() else {
            return Resolved::NotFound;
        };
        let module_walk = self.walk_path_in(module_part, path_accessor, crate_name, in_progress);
        if matches!(module_walk.stop, Some(PathStop::External { .. })) {
            return Resolved::External;
        }
        let Some(module) = module_walk.module() else {
            return Resolved::NotFound;
        };
        match self.resolve_item_in(&module.path, name, accessor, crate_name, in_progress) {
            Resolved::Accessible(item) if !module.accessible => Resolved::Inaccessible(item),
            found => found,
        }
    }

    /// Walk an absolute path segment by segment from the crate root, each segment a module,
    /// and report where the walk stopped and which segment was the first not visible to
    /// `accessor`. This is the only walk along path segments.
    pub fn walk_path(
        &self,
        abs: &[String],
        accessor: &ModulePath,
        crate_name: Option<&str>,
    ) -> PathWalk {
        self.walk_path_in(abs, accessor, crate_name, &mut Walk::default())
    }

    fn walk_path_in(
        &self,
        abs: &[String],
        accessor: &ModulePath,
        crate_name: Option<&str>,
        in_progress: &mut Walk,
    ) -> PathWalk {
        let mut walk = PathWalk {
            reached: ModulePath::root(),
            accessible: true,
            first_private: None,
            stop: None,
        };
        if abs.first().map(String::as_str) != Some("crate") {
            walk.stop = Some(PathStop::Unrooted);
            return walk;
        }
        for (index, segment) in abs.iter().enumerate().skip(1) {
            let mark = in_progress.links.len();
            let found =
                self.resolve_item_in(&walk.reached, segment, accessor, crate_name, in_progress);
            in_progress.links.truncate(mark);
            if matches!(found, Resolved::External) {
                walk.stop = Some(PathStop::External { index });
                return walk;
            }
            if matches!(found, Resolved::Inaccessible(_)) {
                walk.accessible = false;
                walk.first_private.get_or_insert(index);
            }
            let Some(item) = found.item() else {
                walk.stop = Some(PathStop::Missing { index });
                return walk;
            };
            // A path segment is in the type namespace: a module of the same name as a fn
            // or const of the module is the one meant.
            let Some(item) = (item.kind == ItemKind::Module)
                .then_some(item)
                .or_else(|| self.own_module_item(&walk.reached, segment))
            else {
                walk.stop = Some(PathStop::NotModule { index });
                return walk;
            };
            walk.reached = item.module.child(&item.name);
        }
        walk
    }

    /// Whether `module` holds an item-level macro invocation, which may define any item.
    pub fn has_macro_call(&self, module: &ModulePath) -> bool {
        self.modules
            .get(module)
            .or_else(|| self.enclosing_file_module(module))
            .is_some_and(|info| info.macro_modules.contains(module))
    }

    /// The module item `name` declared directly in `module`, if any.
    fn own_module_item(&self, module: &ModulePath, name: &str) -> Option<&ItemInfo> {
        let info = self.modules.get(module).or_else(|| self.enclosing_file_module(module))?;
        info.items
            .iter()
            .find(|i| i.kind == ItemKind::Module && i.name == name && i.module == *module)
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
