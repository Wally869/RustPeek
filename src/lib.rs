pub mod types;
pub mod discovery;
pub mod parser;
pub mod indexer;
pub mod validator;
pub mod fixer;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use types::*;

struct CargoInfo {
    /// With `-` replaced by `_` for Rust identifiers.
    crate_name: Option<String>,
    /// None when Cargo.toml is missing or does not parse: the dependencies are unknown.
    dependencies: Option<HashSet<String>>,
}

fn read_cargo_info(crate_root: &Path) -> CargoInfo {
    let unknown = CargoInfo { crate_name: None, dependencies: None };
    let Ok(content) = std::fs::read_to_string(crate_root.join("Cargo.toml")) else {
        return unknown;
    };
    let Ok(manifest) = content.parse::<toml::Table>() else {
        return unknown;
    };
    let crate_name = manifest
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(|n| n.replace('-', "_"));

    let mut dependencies = HashSet::new();
    let mut collect = |table: &toml::Table| {
        for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(deps) = table.get(kind).and_then(|d| d.as_table()) {
                dependencies.extend(deps.keys().map(|k| k.replace('-', "_")));
            }
        }
    };
    collect(&manifest);
    if let Some(targets) = manifest.get("target").and_then(|t| t.as_table()) {
        for target in targets.values().filter_map(|t| t.as_table()) {
            collect(target);
        }
    }
    CargoInfo { crate_name, dependencies: Some(dependencies) }
}

fn collect_extern_crates(ast: &syn::File, names: &mut HashSet<String>) {
    for item in &ast.items {
        if let syn::Item::ExternCrate(e) = item {
            let name = e.rename.as_ref().map_or(&e.ident, |(_, alias)| alias);
            names.insert(name.to_string());
        }
    }
}

/// Run the full rustpeek analysis on a crate.
///
/// - `crate_root`: path to the crate root directory (containing Cargo.toml)
/// - `changed_files`: optional list of changed files to focus validation on.
///   If None, all files are validated.
pub fn analyze(crate_root: &Path, changed_files: Option<&[PathBuf]>) -> AnalysisResult {
    let crate_files = discovery::discover_crate(crate_root);
    let src_dir = crate_root.join("src");

    // Determine which files to check
    let files_to_check: Vec<(&ModulePath, &PathBuf)> = if let Some(changed) = changed_files {
        let changed_set: HashSet<_> = changed.iter().collect();
        crate_files
            .files
            .iter()
            .filter(|(_, path)| changed_set.contains(path))
            .collect()
    } else {
        crate_files.files.iter().collect()
    };

    // ── Pass 1: Syntax validation ──
    let mut all_diagnostics = Vec::new();
    let mut has_syntax_errors = false;

    let mut parsed_files: Vec<(&ModulePath, syn::File, &PathBuf)> = Vec::new();

    for (module_path, file_path) in &files_to_check {
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(e) => {
                all_diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    file: file_path.to_path_buf(),
                    line: 0,
                    column: 0,
                    message: format!("could not read file: {e}"),
                    error_code: None,
                    hint: None,
                    fix: None,
                });
                continue;
            }
        };

        let syntax_errors = parser::check_syntax(file_path, &source);
        if !syntax_errors.is_empty() {
            has_syntax_errors = true;
            all_diagnostics.extend(syntax_errors);
            continue;
        }

        if let Some(ast) = parser::parse_file(&source) {
            parsed_files.push((module_path, ast, file_path));
        }
    }

    // If there are syntax errors, stop here — don't run Pass 2
    if has_syntax_errors {
        return AnalysisResult {
            diagnostics: all_diagnostics,
        };
    }

    // ── Pass 2: Crate indexing + validation ──

    let CargoInfo { crate_name, dependencies } = read_cargo_info(crate_root);
    let mut extern_crates = HashSet::new();

    // Step 1: Build the symbol table from ALL files in the crate
    let mut symbol_table = SymbolTable::new();

    for (module_path, file_path) in &crate_files.files {
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let ast = match parser::parse_file(&source) {
            Some(ast) => ast,
            None => continue,
        };

        collect_extern_crates(&ast, &mut extern_crates);
        let module_info = indexer::index_file(&ast, module_path, file_path);
        symbol_table.modules.insert(module_path.clone(), module_info);
    }

    let external_crates = dependencies.map(|mut deps| {
        deps.extend(extern_crates);
        deps
    });

    // Step 2: Validate only the changed files against the full symbol table
    for (module_path, ast, file_path) in &parsed_files {
        let diagnostics = validator::validate_file(
            ast,
            file_path,
            module_path,
            &symbol_table,
            &src_dir,
            crate_name.as_deref(),
            external_crates.as_ref(),
        );
        all_diagnostics.extend(diagnostics);
    }

    AnalysisResult {
        diagnostics: all_diagnostics,
    }
}
