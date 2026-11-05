pub mod scope {
    pub mod inner {
        pub(in crate::pub_in_errors::scope) fn restricted() {}
    }
}
pub mod outside {
    use crate::pub_in_errors::scope::inner::restricted;
}
