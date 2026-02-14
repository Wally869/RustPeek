pub mod types;
pub mod discovery;
pub mod parser;
pub mod indexer;
pub mod validator;
pub mod fixer;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use types::*;

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

        let module_info = indexer::index_file(&ast, module_path, file_path);
        symbol_table.modules.insert(module_path.clone(), module_info);
    }

    // Step 2: Validate only the changed files against the full symbol table
    for (module_path, ast, file_path) in &parsed_files {
        let diagnostics =
            validator::validate_file(ast, file_path, module_path, &symbol_table, &src_dir);
        all_diagnostics.extend(diagnostics);
    }

    AnalysisResult {
        diagnostics: all_diagnostics,
    }
}
