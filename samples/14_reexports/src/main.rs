extern crate alloc;
// A root `extern crate` alias joins the extern prelude: child modules use it unqualified.
extern crate core as root_core;
mod ast;
mod path_calls;
mod macro_items;
// A cfg pair: only the active file must exist.
#[cfg(windows)]
#[path = "cfg_twin/windows.rs"]
mod cfg_twin;
#[cfg(not(windows))]
#[path = "cfg_twin/missing.rs"]
mod cfg_twin;
mod extern_paths;
mod a;
mod other;
mod cyc;
mod twohop;
mod globs;
mod inline_mods;
mod crate_vis;
mod mod_privacy;
mod glob_privacy;
mod pub_in;
mod reexport_vis;
mod fn_uses;
mod qualified_paths;
mod bare_globs;
mod reexport_chain;
mod pub_in_nested;
mod bare_paths;
mod unknown_globs;
mod variant_reexports;
mod glob_private_names;
mod expr_privacy;
mod known_uses;
mod shadowed_names;
mod nested_path;
mod lowercase_names;
#[path = "path_mapped/real_name.rs"]
mod renamed;
use crate::crate_vis::C;
use crate::inline_mods::hub::Packet;
use crate::inline_mods::named::Frame;
use crate::inline_mods::empty;
use crate::inline_mods::empty::*;
use crate::cyc::Leaf;
use crate::cyc::Top;
fn main() { let _ = (Leaf, Top); }
