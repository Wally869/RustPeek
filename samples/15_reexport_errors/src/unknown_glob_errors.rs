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
