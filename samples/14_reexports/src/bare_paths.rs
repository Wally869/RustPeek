mod inner {
    pub struct Pub;
    pub mod open {
        pub struct Y;
    }
}

mod filemod;

use inner::Pub;
use inner::open::Y;
use filemod::FPub;
use filemod::open::FY;

mod nested {
    mod deep {
        pub struct D;
    }
    use deep::D;

    pub fn make() -> D {
        use deep::D as Again;
        let _x: deep::D = Again;
        D
    }
}

fn in_body() {
    use inner::Pub;
    use filemod::open::FY;
    let _a = inner::open::Y;
    let _b = filemod::FPub;
    let _c: inner::Pub = Pub;
    let _d = FY;
}
