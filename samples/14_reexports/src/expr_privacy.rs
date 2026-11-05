mod inner {
    pub struct Pub;
    mod hidden {
        pub struct X;
    }
    pub mod open {
        pub struct Y;
        pub(super) struct S;
    }
    pub enum E {
        A,
    }
    pub fn inside() {
        let _ = open::S;
        let _ = hidden::X;
    }
}

mod sub {
    fn from_sub() {
        let _ = super::inner::Pub;
        let _ = super::inner::E::A;
    }
}

fn paths() {
    let _ = inner::E::A;
    let _ = inner::open::Y;
    let _ = inner::Pub;
    let _ = crate::expr_privacy::inner::Pub;
    let _ = self::inner::open::Y;
}
