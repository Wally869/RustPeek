mod inner;
use self::inner::P;
pub(self) use self::inner::P as R;
pub mod open {
    pub mod sub {
        struct Hidden;
    }
}
pub use self::open::sub as alias;
