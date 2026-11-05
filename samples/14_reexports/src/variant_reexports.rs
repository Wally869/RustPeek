mod m {
    pub enum G {
        Y,
    }
    pub use self::inner::H::Z;
    mod inner {
        pub enum H {
            Z,
        }
    }
    enum E {
        A,
    }
    use self::E::A;
}

pub use m::G::Y;
