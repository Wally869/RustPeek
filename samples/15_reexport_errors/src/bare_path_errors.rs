mod inner {
    struct Priv;
    pub struct Pub;
    mod hidden {
        pub struct X;
    }
    pub mod open {
        pub struct Y;
        struct Z;
    }
}

mod filemod;

use inner::Priv;
use inner::hidden::X;
use inner::open::Z;
use filemod::FPriv;
use filemod::hidden::FX;
use filemod::open::FZ;

fn missing_module_in_expr() {
    let _ = inner::gone::X;
}
