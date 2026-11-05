// Bare globs of inline modules: `use inner::*` with no `self::` prefix.
mod inner {
    pub struct Alpha;

    pub mod nested {
        pub struct Beta;
    }

    use nested::*;

    fn in_inner() {
        let _b: Beta = Beta;
    }
}

pub mod outer {
    pub mod mid {
        pub struct Gamma;
    }

    pub use mid::*;
}

use inner::*;
use outer::*;

fn run() {
    let _a: Alpha = Alpha;
    let _g: Gamma = Gamma;
}

fn in_body() {
    use inner::*;
    let _a: Alpha = Alpha;
}

mod user {
    use super::outer::Gamma;

    fn take() {
        let _g: Gamma = Gamma;
    }
}
