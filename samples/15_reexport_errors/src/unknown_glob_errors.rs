use missing::*;

mod nested {
    mod deeper {
        use missing::*;
    }
}

fn in_body() {
    use missing::*;
}

// `alloc` needs `extern crate alloc;`, which this crate does not have (E0433:
// more segments follow the unknown crate).
use alloc::vec::*;

// A non-root `extern crate` alias is in scope only in its declaring module:
// a child module cannot use it unqualified (E0433).
mod declares_alias {
    extern crate core as kore;
    pub fn here() { kore::mem::drop(1u8); }
    mod child {
        pub fn there() { kore::mem::drop(2u8); }
    }
}
