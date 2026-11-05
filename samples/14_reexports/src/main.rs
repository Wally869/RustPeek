mod ast;
mod a;
mod other;
mod cyc;
mod twohop;
mod globs;
use crate::cyc::Leaf;
use crate::cyc::Top;
fn main() { let _ = (Leaf, Top); }
