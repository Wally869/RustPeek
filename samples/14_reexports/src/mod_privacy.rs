pub mod a {
    mod hidden {
        pub struct Item;
        pub mod deep {
            pub struct Deep;
            use crate::mod_privacy::a::hidden::Item as FromDescendant;
        }
    }

    pub use self::hidden::Item as Exported;
    use crate::mod_privacy::a::hidden::Item as FromParent;
    use hidden::deep::Deep as FromParentDeep;

    pub mod sibling {
        use crate::mod_privacy::a::hidden::Item as FromSibling;
        use super::hidden::deep::Deep as FromSiblingDeep;
    }

    pub mod globbed {
        pub use super::hidden::*;
    }
}

pub mod outside_user {
    use crate::mod_privacy::a::Exported;
    use crate::mod_privacy::a::globbed::Item;
}
