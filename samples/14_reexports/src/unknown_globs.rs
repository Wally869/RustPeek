use std::collections::*;
use core::cmp::*;
use alloc::vec::*;
use serde::*;
use proc_macro2::*;

pub enum Color {
    Red,
}

pub mod m {
    pub enum G {
        A,
    }
}

use Color::*;

mod sub {
    use crate::unknown_globs::m;
    use m::G::*;
}

fn in_body() {
    use std::fmt::*;
    use crate::unknown_globs::m;
    use m::G::*;
}
