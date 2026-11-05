pub struct Base;
mod k {
    pub struct K1;
}
use self::k::*;
mod child;
pub fn d(_: K1, _: Base) {}
