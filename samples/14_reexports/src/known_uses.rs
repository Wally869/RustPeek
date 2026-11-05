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

// Round 9: a prelude enum as the root of a `use` (itertools multi_product.rs).
mod prelude_roots {
    use Option::{self as Maybe, None as Nothing, Some as Just};
    use Result::{Err as Break, Ok as Continue};

    pub fn f() -> Maybe<u8> {
        let _: Result<u8, u8> = if true { Continue(1) } else { Break(2) };
        if false { Nothing } else { Just(1) }
    }
}
