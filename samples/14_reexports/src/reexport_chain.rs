mod def {
    pub struct Item;
    pub mod Sub {}
}

mod mid {
    pub use crate::reexport_chain::def::Item;
    pub use crate::reexport_chain::def::Sub;
}

mod outer {
    pub use crate::reexport_chain::mid::Item;
    pub use crate::reexport_chain::mid::Sub;
}

mod glob_mid {
    pub use crate::reexport_chain::def::*;
}

mod glob_outer {
    pub use crate::reexport_chain::glob_mid::Item;
    pub use crate::reexport_chain::glob_mid::Sub;
}

mod crate_mid {
    pub(crate) use crate::reexport_chain::def::Item;
}

mod crate_outer {
    pub(crate) use crate::reexport_chain::crate_mid::Item;
}

mod private_mid {
    mod src {
        pub struct Thing;
    }
    use self::src::Thing;
    mod inner {
        use super::Thing;
        pub(super) use super::Thing as Again;
    }
}
