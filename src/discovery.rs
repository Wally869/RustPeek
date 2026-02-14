use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::types::ModulePath;

/// Discovered .rs file mapped to its module path
#[derive(Debug)]
pub struct CrateFiles {
    /// Map from module path to file path
    pub files: HashMap<ModulePath, PathBuf>,
    /// The crate root directory
    pub root: PathBuf,
}

/// Discover all .rs files in a crate and map them to module paths.
pub fn discover_crate(root: &Path) -> CrateFiles {
    let src_dir = root.join("src");
    let mut files = HashMap::new();

    // Find the crate root file
    let lib_rs = src_dir.join("lib.rs");
    let main_rs = src_dir.join("main.rs");

    let crate_root = if lib_rs.exists() {
        lib_rs
    } else if main_rs.exists() {
        main_rs
    } else {
        return CrateFiles {
            files,
            root: root.to_path_buf(),
        };
    };

    // Add the crate root
    files.insert(ModulePath::root(), crate_root);

    // Walk the src directory for all .rs files
    for entry in WalkDir::new(&src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().is_some_and(|ext| ext == "rs")
                && e.file_type().is_file()
        })
    {
        let path = entry.path().to_path_buf();

        // Skip crate root files, already handled
        if path == src_dir.join("lib.rs") || path == src_dir.join("main.rs") {
            continue;
        }

        // Convert file path to module path
        if let Some(module_path) = file_to_module_path(&src_dir, &path) {
            files.insert(module_path, path);
        }
    }

    CrateFiles {
        files,
        root: root.to_path_buf(),
    }
}

/// Convert a file path relative to src/ into a module path.
///
/// Examples:
///   src/parser.rs         -> crate::parser
///   src/parser/mod.rs     -> crate::parser
///   src/parser/utils.rs   -> crate::parser::utils
fn file_to_module_path(src_dir: &Path, file: &Path) -> Option<ModulePath> {
    let relative = file.strip_prefix(src_dir).ok()?;
    let mut segments = vec!["crate".to_string()];

    let components: Vec<_> = relative.components().collect();

    for (i, component) in components.iter().enumerate() {
        let name = component.as_os_str().to_str()?;

        if i == components.len() - 1 {
            // Last component is the filename
            let stem = Path::new(name).file_stem()?.to_str()?;
            if stem != "mod" {
                segments.push(stem.to_string());
            }
        } else {
            segments.push(name.to_string());
        }
    }

    Some(ModulePath(segments))
}

/// Resolve a `mod foo;` declaration to its file path.
/// Returns the path if the file exists, None otherwise.
pub fn resolve_mod_file(src_dir: &Path, parent_module: &ModulePath, mod_name: &str) -> Option<PathBuf> {
    // Build the directory path for the parent module
    let mut dir = src_dir.to_path_buf();
    for segment in &parent_module.0[1..] {
        // skip "crate"
        dir = dir.join(segment);
    }

    // Check foo.rs (sibling style)
    let sibling = dir.with_extension("").join(format!("{mod_name}.rs"));

    // For the root module, check directly in src/
    let direct = if parent_module.0.len() == 1 {
        src_dir.join(format!("{mod_name}.rs"))
    } else {
        // For nested modules, the parent's directory is named after the parent
        let parent_name = parent_module.last();
        let parent_dir = dir.parent()?.join(parent_name);
        parent_dir.join(format!("{mod_name}.rs"))
    };

    // Check foo/mod.rs style
    let mod_style = if parent_module.0.len() == 1 {
        src_dir.join(mod_name).join("mod.rs")
    } else {
        let parent_name = parent_module.last();
        let parent_dir = dir.parent()?.join(parent_name);
        parent_dir.join(mod_name).join("mod.rs")
    };

    if direct.exists() {
        Some(direct)
    } else if mod_style.exists() {
        Some(mod_style)
    } else if sibling.exists() {
        Some(sibling)
    } else {
        None
    }
}
