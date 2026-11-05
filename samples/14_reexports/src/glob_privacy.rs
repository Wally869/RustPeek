mod inner {
    struct Hidden;
    pub struct Shown;
    pub fn shown_fn() {}
    fn hidden_fn() {}
}

use self::inner::*;

pub struct Holder {
    pub shown: Shown,
}

pub mod m {
    pub use self::inner::*;

    mod inner {
        pub(super) struct Sup;
        pub struct Pub;
    }

    pub struct User {
        pub sup: Sup,
        pub p: Pub,
    }
}
