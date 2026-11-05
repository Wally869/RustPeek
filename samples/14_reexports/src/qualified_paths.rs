pub mod m {
    pub struct Ty;
    impl Ty {
        pub fn new() -> Ty {
            Ty
        }
    }
    pub enum E {
        V,
    }
    pub fn f() {}
}

pub mod sibling {
    pub fn f() {}
}

mod user {
    pub mod inner {
        pub const C: u32 = 1;
    }

    fn use_all(_: crate::qualified_paths::m::Ty) {
        super::sibling::f();
        crate::qualified_paths::m::f();
        let _ = crate::qualified_paths::m::Ty::new();
        let _ = self::inner::C;
        let _ = crate::qualified_paths::m::E::V;
    }
}
