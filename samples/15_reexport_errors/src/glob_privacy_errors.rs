pub mod m {
    pub use self::inner::*;

    mod inner {
        pub(super) struct Sup;
        pub struct Pub;
        struct Hidden;
    }
}

use crate::glob_privacy_errors::m::Sup;
use crate::glob_privacy_errors::m::Hidden;
use crate::glob_privacy_errors::m::Pub;
