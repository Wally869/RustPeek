mod m {
    enum E {
        A,
        B(u8),
        C { x: u8 },
    }
    pub use self::E::A;
    pub use self::E::B;
    pub use self::E::C;
    pub(crate) use self::E::A as A2;

    pub(crate) enum F {
        X,
    }
    pub enum G {
        Y,
    }
}

pub use m::F::X;

pub(crate) enum K {
    V,
}
pub use K::V;

use m::E::A as PrivateEnumVariant;
use m::G::Q;
