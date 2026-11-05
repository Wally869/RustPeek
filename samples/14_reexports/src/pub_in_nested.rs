pub(in super) fn up_one() {}
pub(super) fn up_one_short() {}
pub(in crate::pub_in_nested) fn here() {}

mod outer {
    mod inner {
        pub(in super::super) fn up_two() {}
        pub(in crate::pub_in_nested) fn from_root() {}
        pub(in super) fn up_one() {}
    }
}
