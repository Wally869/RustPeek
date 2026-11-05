pub mod a {
    mod hidden {
        pub struct Item;
    }
}

pub mod outside {
    use crate::mod_privacy_errors::a::hidden::Item;
    use crate::mod_privacy_errors::a::hidden;
    use crate::mod_privacy_errors::a::hidden::*;
}
