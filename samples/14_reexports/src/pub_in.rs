pub mod scope {
    pub mod inner {
        pub(in crate::pub_in::scope) fn restricted() {}
        pub(in super) fn parent_only() {}
        pub(in self) fn own_only() {}
    }
    pub mod sibling {
        use crate::pub_in::scope::inner::restricted;
        use super::inner::parent_only;
        pub mod deeper {
            use crate::pub_in::scope::inner::restricted;
        }
    }
    use self::inner::restricted;
    use inner::parent_only;
}
