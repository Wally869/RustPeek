mod inner {
    struct Priv;
    pub struct Pub;
    struct PrivT {}
    mod hidden {
        pub struct X;
        pub fn f() {}
    }
    pub mod open {
        struct Z;
        fn g() {}
        pub struct Y;
        pub(super) struct S;
    }
    pub enum E {
        A,
    }
    enum PE {
        B,
    }
}

mod sub {
    fn from_sub() {
        let _ = super::inner::Priv;
    }
}

fn paths() {
    let _ = inner::Priv;
    let _ = self::inner::Priv;
    let _ = crate::expr_privacy_errors::inner::hidden::X;
    inner::hidden::f();
    inner::open::g();
    let _t: inner::PrivT;
    let _ = inner::PE::B;
    let _ = inner::open::Z;
    let _ = inner::PrivT {};
    let _ = inner::open::S;
}
