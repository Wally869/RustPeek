use std::path::Path;

use crate::types::{Diagnostic, Severity};

/// Pass 1: Parse a file with syn and return syntax errors if any.
pub fn check_syntax(file_path: &Path, source: &str) -> Vec<Diagnostic> {
    match syn::parse_file(source) {
        Ok(_) => Vec::new(),
        Err(err) => {
            let span = err.span();
            vec![Diagnostic {
                severity: Severity::Error,
                file: file_path.to_path_buf(),
                line: span.start().line,
                column: span.start().column + 1,
                message: format!("syntax error: {err}"),
                error_code: None,
                hint: None,
                fix: None,
            }]
        }
    }
}

/// Parse a file and return the AST, or None if it has syntax errors.
pub fn parse_file(source: &str) -> Option<syn::File> {
    syn::parse_file(source).ok()
}
