// A `#[macro_export]` macro lives at the crate root: `crate::m` and `<crate name>::m` name it.
mod defs;

mod by_crate {
    use crate::exported_in_child;

    pub fn run() -> u32 {
        exported_in_child!()
    }
}

mod by_name {
    use sample_reexports::exported_in_child;

    pub fn run() -> u32 {
        exported_in_child!()
    }
}

pub fn both() -> u32 {
    by_crate::run() + by_name::run()
}
