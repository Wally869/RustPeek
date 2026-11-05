#![allow(dead_code, unexpected_cfgs)]
// Items that share a name and differ by `#[cfg]` are alternatives: a check passes when any
// alternative passes.

#[cfg(feature = "full")]
fn unary(a: u8, b: u8) -> u8 {
    a + b
}
#[cfg(not(feature = "full"))]
fn unary(a: u8) -> u8 {
    a
}

#[cfg(feature = "full")]
pub fn call_full() -> u8 {
    unary(1, 2)
}
#[cfg(not(feature = "full"))]
pub fn call_minimal() -> u8 {
    unary(1)
}

mod left {
    pub fn pick() -> u8 {
        1
    }
}
mod right {
    pub fn pick() -> u8 {
        2
    }
}
#[cfg(feature = "full")]
use self::left::pick;
#[cfg(not(feature = "full"))]
use self::right::pick;

pub fn use_pick() -> u8 {
    pick()
}

mod kv {
    pub struct Error;

    pub mod value {
        use crate::cfg_alternatives::kv::Error;

        #[cfg(feature = "x")]
        pub(in crate::cfg_alternatives::kv) mod inner {
            pub use core::fmt::Error;
        }
        #[cfg(not(feature = "x"))]
        pub(in crate::cfg_alternatives::kv) mod inner {
            use super::*;
            pub fn g(_: Error) {}
        }
    }

    pub mod error {
        #[cfg(feature = "x")]
        pub(super) fn f(_: crate::cfg_alternatives::kv::value::inner::Error) {}
    }
}

pub struct Span {
    #[cfg(feature = "locs")]
    lo: u32,
}

impl Span {
    #[cfg(not(feature = "locs"))]
    pub fn new() -> Span {
        Span {}
    }
    #[cfg(feature = "locs")]
    pub fn new() -> Span {
        Span { lo: 0 }
    }
}

// Round 10 review: a parameter under `#[cfg]` makes the arity unknown (syn
// `parse_binop_rhs`): the call with the cfg-off arity is valid.
fn binop_rhs(input: u8, #[cfg(feature = "full")] allow_struct: bool, precedence: u8) -> u8 {
    #[cfg(feature = "full")]
    let _ = allow_struct;
    input + precedence
}

pub fn call_binop_rhs() -> u8 {
    binop_rhs(1, 2)
}

// Round 10 review: a `#[cfg] mod x;` pair without `#[path]`; both name the same file.
#[cfg(feature = "full")]
mod filemod;
#[cfg(not(feature = "full"))]
mod filemod;

pub fn call_filemod() -> u8 {
    filemod::value()
}
