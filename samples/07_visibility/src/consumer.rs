// Good — public item
use crate::inner::PublicThing;

// E0603: Private struct from another module
use crate::inner::PrivateStruct;

// Good — pub(crate) is accessible within the crate
use crate::inner::crate_visible;

// E0603: Private function from another module
use crate::inner::private_fn;

pub fn consume() {
    let _p = PublicThing { name: "hi".to_string() };
    let _n = crate_visible();
}
