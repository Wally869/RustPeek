use serde::Serialize;
use proc_macro2::TokenStream;
use std::collections::HashMap;
use core::cmp::Ordering;
use alloc::vec::Vec as AllocVec;

pub enum Shade {
    Dark,
    Light,
}

use Shade::Dark;

mod sub {
    use std::fmt::Debug;
    use core::fmt::Display;
}

fn in_body() {
    use std::fmt::Write;
    use serde::Deserialize;
    use Shade::Light;
}
