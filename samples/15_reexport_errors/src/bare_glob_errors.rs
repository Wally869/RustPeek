pub mod inner {
    mod hidden {
        pub struct H;
    }

    pub mod open {
        pub struct O;
    }
}

use inner::hidden::*;
use inner::gone::*;
use inner::open::*;
use std::collections::*;
