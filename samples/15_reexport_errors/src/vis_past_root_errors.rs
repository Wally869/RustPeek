pub(in super::super) fn file_item() {}

pub struct Holder {
    pub(in super::super) field: u32,
}

mod inner {
    pub(in super::super::super) fn inline_item() {}
}
