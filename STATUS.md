# RustPeek status

Goal: a CLI and library that checks a Rust crate for unresolved `use` paths and names, and fixes them.
Use: catch and fix AI-written code errors without running rustc.
Must have: no false error on valid Rust, the right error code on invalid Rust.

## Current state
- main at 6b2ac61. Rounds 1-10 landed (round files in `temp/round-<n>/`).
- Resolver in `src/types.rs`, one path walker (`walk_path`) in `src/validator/` (7 files). Covers `super`/`self`/`crate` paths, bare local paths, `pub use` chains, globs, inline modules, visibility and privacy (E0603, E0364/E0365), fn-body uses, enum variants, unknown first segments, `extern crate` names (crate-root scope), `#[path]`, `#[cfg]` and `#[cfg_attr(.., path)]` mods, `#[cfg]` alternatives (same name under different cfgs; a cfg-gated parameter makes the arity unknown), per-namespace lookup (Type/Value/Any), `#[macro_export]` macros as crate-root items, lowercase names behind globs, missing associated items (E0599).
- Macro rule: a name is not reported in a module, block or trait body that holds an item-like macro call, or behind a glob of such a module.
- Tests: sample crates `samples/01..16` (`run_samples.bat`). Sample 14 is valid Rust (rustc accepts it) and must report 0; sample 15 reports 105 errors + 6 suggestions by design (checked against rustc 1.101); sample 16 is the check, fix, recheck sample.
- Real-crate measure (31 valid crates from the cargo registry, `temp/round-7/survey.md` and `temp/round-9/survey.md`): 0 errors and 0 suggestions on all of them after round 10.

## Next steps
1. A crate with a `cfg_attr(.., path)` mod gets no E0599 anywhere (the macro rule applies to the whole crate).
2. In a group of same-name cfg alternatives, if one fn has a cfg-gated parameter, the arity check still runs against the other alternative.
3. No E0599 when the type has an impl of a non-local trait or is reached through a type alias (limit, no false report; needs the method lists of std and dependency traits, so it stays).
4. Widen the survey to more crates, and re-run it after each change.

## Waits for the user
Nothing. No round is queued.

## Resume
Read this file and `.claude/CLAUDE.md`. Next round number: 11. Main is checked out in the main checkout: a lead hands back `Not landed:` and the CTO fast-forwards. Lead worktrees from rounds 6-10 are still in `.claude/worktrees/`, locked.
