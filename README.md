# rustpeek

Fast Rust validation without compilation. Parses `.rs` files with `syn`, builds a crate-level symbol table, catches structural errors. No `rustc`, no `target/`, sub-second.

> **Disclaimer:** This codebase is AI-generated. Rust's module and name resolution has many edge cases, and the logic here may not handle all of them correctly. Use `check` freely, but exercise caution with `fix` — review what it changes, and keep your work committed before running it.

## Usage

```
rustpeek check <crate-path> [changed-files...]
rustpeek fix   <crate-path> [changed-files...]
```

`check` reports errors and suggestions. `fix` auto-applies obvious fixes (missing imports, import typos) and reports the rest. Add `--json` for machine-readable output.

## What It Catches

| Check | Code | Output | Auto-fix |
|---|---|---|---|
| `mod foo;` with no file | E0583 | error | no |
| `use crate::foo::Bar` — Bar doesn't exist | E0432 | error | yes (typo correction) |
| `use crate::missing::X` — module doesn't exist | E0433 | error | no |
| Missing fields in struct literal | E0063 | error | no |
| Nonexistent field in struct literal | E0609 | error | no |
| Nonexistent enum variant | E0599 | error | no |
| Wrong number of function arguments | E0061 | error | no |
| Accessing private items cross-module | E0603 | error | no |
| Type exists in crate but not imported | E0412 | suggestion | yes (inserts `use`) |

## What It Ignores

| Category | Why |
|---|---|
| External dependencies | Can't know what other crates export |
| Trait methods | `.to_string()` etc. require trait solving |
| Macros | `derive`, `macro_rules!` output is opaque |
| Type inference | `let x = foo()` — no idea what type `x` is |
| Borrow checking / lifetimes | Requires full compiler analysis |
| Generics / where clauses | Requires monomorphization |

If it can't be proven wrong from source files alone, rustpeek says nothing.

## Library

```rust
// Check all files
let result = rustpeek::analyze(Path::new("./my-crate"), None);

// Check only specific files (still indexes the full crate for context)
let changed = vec![PathBuf::from("src/handlers.rs")];
let result = rustpeek::analyze(Path::new("./my-crate"), Some(&changed));

for diag in &result.diagnostics { println!("{diag}"); }
```

The second argument is an optional list of changed files. Pass `None` to validate everything, or `Some(&[...])` to only validate those files — useful for incremental checks where you only care about what you just edited.

## Testing

```
run_samples.bat     # check all samples
test_fix.bat        # check → fix → recheck cycle
```
