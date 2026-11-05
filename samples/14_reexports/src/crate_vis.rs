mod hidden {
    pub struct C;
}

pub(crate) use self::hidden::C;

pub mod sup {
    mod deep {
        pub struct S;
    }
    pub(super) use self::deep::S;
}

use self::sup::S;
