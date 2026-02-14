# rustpeek

Syn-based Rust crate indexer for lightweight validation without compilation.

## Codemap

```
src/
├── lib.rs          # Public API: analyze(crate_root, changed_files) -> AnalysisResult
├── types.rs        # Core types: ModulePath, SymbolTable, ItemInfo, Diagnostic, Fix, Vis
├── discovery.rs    # File discovery: walks src/, maps .rs files to module paths
├── parser.rs       # Pass 1: syn::parse_file() syntax validation
├── indexer.rs      # Pass 2a: builds symbol table (structs, enums, fns, impls, use stmts)
├── validator.rs    # Pass 2b: validates references against symbol table, generates fixes
├── fixer.rs        # Applies Fix objects (insert/replace/remove lines) to source files
└── main.rs         # CLI: rustpeek [check|fix] [--json] <crate-path> [changed-files...]

samples/            # Test crates, one per feature (01_syntax_errors through 13_not_imported)
run_samples.bat     # Run check against all samples
test_fix.sh/.bat    # check → fix → recheck cycle on all samples
```

## Key design rule

Only flag what's provably wrong from source files alone. If a symbol could come from an external dep, macro, or trait impl — stay silent. False positives are worse than missed errors.
