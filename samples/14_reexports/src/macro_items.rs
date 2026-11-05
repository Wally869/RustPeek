// T5: an enum struct variant is not the struct of the same name; an item made by a macro
// invocation in the module is not reported.

pub struct Map {
    pub map: u8,
}

pub enum SerializeMap {
    Map { map: u8, next_key: Option<u8> },
}

pub fn make() -> SerializeMap {
    let _ = Map { map: 1 };
    SerializeMap::Map { map: 1, next_key: None }
}

macro_rules! make_struct {
    ($name:ident) => {
        pub struct $name;
    };
}

mod other {
    pub trait Flags {}
}

mod generated {
    make_struct!(Flags);

    pub fn take(_flag: Flags) {}
}
