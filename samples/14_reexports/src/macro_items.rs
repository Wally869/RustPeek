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

// A fn and a mod of the same name: the fn is in the value namespace, the mod in the type
// namespace.
mod spans {
    pub fn get_span() -> get_span::GetSpan {
        get_span::GetSpan
    }

    pub mod get_span {
        pub struct GetSpan;
    }

    use self::get_span::GetSpan;

    pub fn twice() -> GetSpan {
        let _ = get_span();
        self::get_span::GetSpan
    }
}

macro_rules! make_unit {
    ($name:ident) => {
        pub struct $name;
        impl $name {
            pub fn make() -> $name {
                $name
            }
        }
    };
}

// A macro invoked in a fn body defines items of that block (bitflags tests).
pub fn block_macro() {
    make_unit! { Local }
    let _ = Local::make();
}

// A glob of a module whose macro invocation defines the name.
mod glob_of_generated {
    use super::made::*;

    pub fn take() -> Made {
        Made::make()
    }
}

mod made {
    make_unit!(Made);
}

mod generated {
    make_struct!(Flags);

    pub fn take(_flag: Flags) {}
}
