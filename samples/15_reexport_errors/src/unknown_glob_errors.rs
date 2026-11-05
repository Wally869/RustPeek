use missing::*;

mod nested {
    mod deeper {
        use missing::*;
    }
}

fn in_body() {
    use missing::*;
}

// `alloc` needs `extern crate alloc;`, which this crate does not have.
use alloc::vec::*;
