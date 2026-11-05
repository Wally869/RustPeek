use missing::Thing;
use missing::sub::Other;

pub struct S;

impl S {
    pub fn new() -> S {
        S
    }
}

pub trait Tr {
    fn method();
}

pub type Alias = S;

use S::new;
use Tr::method;
use Tr::missing_member;
use Alias::new as alias_new;

// A macro call in a trait body may define the member: RustPeek does not report it
// (rustc: E0658 when the macro defines it, E0432 when not; the code is unknown).
macro_rules! trait_member {
    ($n:ident) => { fn $n(); };
}
pub trait MacroTr {
    trait_member!(made);
}
use MacroTr::made;

fn in_body() {
    use missing::Thing as BodyThing;
}
