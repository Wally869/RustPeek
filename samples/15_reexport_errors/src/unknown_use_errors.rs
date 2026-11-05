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

fn in_body() {
    use missing::Thing as BodyThing;
}
