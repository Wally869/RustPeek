// All of these reference external crates.
// Rustpeek should stay COMPLETELY SILENT — even if names are wrong.
// External deps are not our problem.

use serde::Serialize;
use serde::Deserialize;
use serde::nonexistent::Fake;  // typo in external dep — NOT our job

use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Read;

// External trait method — should be silent
fn external_method(v: Vec<i32>) -> String {
    format!("{v:?}")
}

// Type from external crate — should not flag
fn use_hashmap() -> HashMap<String, i32> {
    HashMap::new()
}
