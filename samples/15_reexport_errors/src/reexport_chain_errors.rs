mod def {
    pub struct Item;
    pub mod Sub {}
}

mod narrow_mid {
    pub(crate) use crate::reexport_chain_errors::def::Item;
    pub(crate) use crate::reexport_chain_errors::def::Sub;
}

mod narrow_outer_item {
    pub use crate::reexport_chain_errors::narrow_mid::Item;
}

mod narrow_outer_module {
    pub use crate::reexport_chain_errors::narrow_mid::Sub;
}

mod narrow_glob_mid {
    pub(crate) use crate::reexport_chain_errors::def::*;
}

mod narrow_glob_outer {
    pub use crate::reexport_chain_errors::narrow_glob_mid::Item;
}

mod private_mid {
    mod src {
        pub struct Thing;
    }
    use self::src::Thing;
    mod inner {
        pub(crate) use super::Thing;
    }
}
