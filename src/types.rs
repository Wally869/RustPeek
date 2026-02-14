use std::collections::HashMap;
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
            Vis::Private => {
                // Private items are accessible within the same module and child modules
                accessor_module.0.starts_with(&defining_module.0)
                    || defining_module.0.starts_with(&accessor_module.0)
            }
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

    /// Find an item by name across all modules
    pub fn find_item(&self, name: &str) -> Vec<&ItemInfo> {
        let mut results = Vec::new();
        for module_info in self.modules.values() {
            for item in &module_info.items {
                if item.name == name {
                    results.push(item);
                }
            }
        }
        results
    }

    /// Find an item in a specific module by name
    pub fn find_in_module(&self, module: &ModulePath, name: &str) -> Option<&ItemInfo> {
        self.modules
            .get(module)
            .and_then(|m| m.items.iter().find(|i| i.name == name))
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
