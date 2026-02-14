use std::path::PathBuf;
use std::process;

use rustpeek::types::{AnalysisResult, Severity};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let json_mode = args.iter().any(|a| a == "--json");
    let positional: Vec<&String> = args.iter().skip(1).filter(|a| !a.starts_with("--")).collect();

    if positional.is_empty() {
        print_usage();
        process::exit(2);
    }

    let subcommand = positional[0].as_str();
    let (fix_mode, path_start) = match subcommand {
        "check" => (false, 1),
        "fix" => (true, 1),
        // No subcommand — treat first arg as crate path (backwards compat)
        _ => (false, 0),
    };

    if positional.len() <= path_start {
        print_usage();
        process::exit(2);
    }

    let crate_root = PathBuf::from(positional[path_start]);

    if !crate_root.join("Cargo.toml").exists() {
        eprintln!("error: no Cargo.toml found in {}", crate_root.display());
        process::exit(2);
    }

    let changed_files: Option<Vec<PathBuf>> = if positional.len() > path_start + 1 {
        Some(positional[path_start + 1..].iter().map(PathBuf::from).collect())
    } else {
        None
    };

    let result = rustpeek::analyze(&crate_root, changed_files.as_deref());

    if fix_mode {
        run_fix(result, json_mode);
    } else {
        run_check(result, json_mode);
    }
}

fn print_usage() {
    eprintln!("Usage: rustpeek [check|fix] [--json] <crate-path> [changed-file ...]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  check   Report errors and suggestions (default)");
    eprintln!("  fix     Auto-apply obvious fixes, report the rest");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --json  Output diagnostics as JSON");
    eprintln!();
    eprintln!("If no changed files are specified, all .rs files are validated.");
}

fn run_check(result: AnalysisResult, json_mode: bool) {
    if json_mode {
        println!("{}", serde_json::to_string_pretty(&result).unwrap());
        process::exit(if result.has_errors() { 1 } else { 0 });
    }

    if result.diagnostics.is_empty() {
        println!("rustpeek: no issues found");
        process::exit(0);
    }

    print_diagnostics(&result);

    let error_count = result.error_count();
    let suggestion_count = result.suggestion_count();
    let fixable = result.fixable_count();

    if error_count > 0 {
        print!("rustpeek: {error_count} error(s), {suggestion_count} suggestion(s)");
        if fixable > 0 {
            print!(" ({fixable} auto-fixable, run `rustpeek fix`)");
        }
        println!();
        process::exit(1);
    } else {
        print!("rustpeek: {suggestion_count} suggestion(s)");
        if fixable > 0 {
            print!(" ({fixable} auto-fixable, run `rustpeek fix`)");
        }
        println!();
        process::exit(0);
    }
}

fn run_fix(result: AnalysisResult, json_mode: bool) {
    let apply_result = rustpeek::fixer::apply_fixes(&result);

    if json_mode {
        let output = serde_json::json!({
            "fixes_applied": apply_result.fixes_applied,
            "remaining": apply_result.remaining,
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
        process::exit(if apply_result.remaining.has_errors() { 1 } else { 0 });
    }

    if apply_result.fixes_applied > 0 {
        println!("rustpeek: applied {} fix(es)", apply_result.fixes_applied);
        // Show what was fixed
        for diag in &result.diagnostics {
            if let Some(fix) = &diag.fix {
                println!("  fixed: {fix}");
            }
        }
        println!();
    }

    let remaining = &apply_result.remaining;
    if remaining.diagnostics.is_empty() {
        if apply_result.fixes_applied == 0 {
            println!("rustpeek: no issues found");
        } else {
            println!("rustpeek: all issues fixed");
        }
        process::exit(0);
    }

    print_diagnostics(remaining);

    let error_count = remaining.error_count();
    let suggestion_count = remaining.suggestion_count();

    if error_count > 0 {
        println!("rustpeek: {error_count} remaining error(s), {suggestion_count} suggestion(s)");
        process::exit(1);
    } else {
        println!("rustpeek: {suggestion_count} remaining suggestion(s)");
        process::exit(0);
    }
}

fn print_diagnostics(result: &AnalysisResult) {
    let mut errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    let mut suggestions: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Suggestion)
        .collect();

    errors.sort_by(|a, b| (&a.file, a.line, a.column).cmp(&(&b.file, b.line, b.column)));
    suggestions.sort_by(|a, b| (&a.file, a.line, a.column).cmp(&(&b.file, b.line, b.column)));

    for diag in &errors {
        println!("{diag}");
        println!();
    }

    for diag in &suggestions {
        println!("{diag}");
        println!();
    }
}
