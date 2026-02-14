use std::collections::HashMap;
use std::path::PathBuf;

use crate::types::{AnalysisResult, Fix};

/// Apply all fixes from an analysis result to the filesystem.
/// Returns the number of fixes applied and the remaining (unfixable) diagnostics.
pub fn apply_fixes(result: &AnalysisResult) -> ApplyResult {
    // Group fixes by file, then sort by line descending so insertions
    // don't shift line numbers for subsequent fixes in the same file.
    let mut fixes_by_file: HashMap<PathBuf, Vec<&Fix>> = HashMap::new();
    let mut applied = 0;

    for diag in &result.diagnostics {
        if let Some(fix) = &diag.fix {
            let file = match fix {
                Fix::InsertLine { file, .. } => file,
                Fix::ReplaceLine { file, .. } => file,
                Fix::RemoveLine { file, .. } => file,
            };
            fixes_by_file.entry(file.clone()).or_default().push(fix);
        }
    }

    for (file_path, mut fixes) in fixes_by_file {
        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut lines: Vec<String> = content.lines().map(String::from).collect();

        // Sort fixes by line number descending so we can apply bottom-up
        // without invalidating line indices
        fixes.sort_by(|a, b| fix_line(b).cmp(&fix_line(a)));

        // Deduplicate: remove InsertLine fixes if a ReplaceLine already
        // corrects the import to that name, and dedup identical inserts
        let mut seen_inserts = std::collections::HashSet::new();
        let mut replace_targets = std::collections::HashSet::new();
        for fix in &fixes {
            if let Fix::ReplaceLine { new_text, .. } = fix {
                replace_targets.insert(new_text.clone());
            }
        }
        fixes.retain(|fix| {
            if let Fix::InsertLine { content, .. } = fix {
                // Skip if this inserts a `use` for a name that a ReplaceLine already fixes
                for target in &replace_targets {
                    if content.contains(&format!("::{target};")) {
                        return false;
                    }
                }
                // Dedup identical inserts
                return seen_inserts.insert(content.clone());
            }
            true
        });

        for fix in &fixes {
            match fix {
                Fix::InsertLine { line, content, .. } => {
                    let idx = if *line == 0 {
                        lines.len()
                    } else {
                        (*line - 1).min(lines.len())
                    };
                    // Insert each line of content (handle trailing newline)
                    let insert_content = content.trim_end_matches('\n');
                    lines.insert(idx, insert_content.to_string());
                    applied += 1;
                }
                Fix::ReplaceLine { line, old_text, new_text, .. } => {
                    let idx = line.saturating_sub(1);
                    if idx < lines.len() && lines[idx].contains(old_text.as_str()) {
                        lines[idx] = lines[idx].replace(old_text.as_str(), new_text);
                        applied += 1;
                    }
                }
                Fix::RemoveLine { line, .. } => {
                    let idx = line.saturating_sub(1);
                    if idx < lines.len() {
                        lines.remove(idx);
                        applied += 1;
                    }
                }
            }
        }

        // Write back
        let new_content = lines.join("\n") + "\n";
        let _ = std::fs::write(&file_path, new_content);
    }

    let remaining = result
        .diagnostics
        .iter()
        .filter(|d| d.fix.is_none())
        .cloned()
        .collect();

    ApplyResult {
        fixes_applied: applied,
        remaining: AnalysisResult {
            diagnostics: remaining,
        },
    }
}

/// Result of applying fixes
pub struct ApplyResult {
    pub fixes_applied: usize,
    pub remaining: AnalysisResult,
}

fn fix_line(fix: &Fix) -> usize {
    match fix {
        Fix::InsertLine { line, .. } => *line,
        Fix::ReplaceLine { line, .. } => *line,
        Fix::RemoveLine { line, .. } => *line,
    }
}

