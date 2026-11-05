// Round 10 review: only a `#[macro_export]` macro is an item of the crate root.
mod defs {
    #[macro_export]
    macro_rules! exported_here {
        () => {};
    }

    #[allow(unused_macros)]
    macro_rules! local_only {
        () => {};
    }
}

mod users {
    #[allow(unused_imports)]
    use crate::exported_here;
    // ERROR: E0432 unresolved import, no macro of that name is exported.
    #[allow(unused_imports)]
    use crate::no_such_macro;
    // ERROR: E0432 unresolved import, `local_only` is not `#[macro_export]`.
    #[allow(unused_imports)]
    use crate::local_only;
}
