mod ast;
mod a;
mod other;
mod cyc;
mod twohop;
mod globs;
mod inline_mods;
use crate::inline_mods::hub::Packet;
use crate::inline_mods::named::Frame;
use crate::inline_mods::empty;
use crate::inline_mods::empty::*;
use crate::cyc::Leaf;
use crate::cyc::Top;
fn main() { let _ = (Leaf, Top); }
