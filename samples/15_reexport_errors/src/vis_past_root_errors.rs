pub(in super::super) fn file_item() {}

pub struct Holder {
    pub(in super::super) field: u32,
}

impl Holder {
    pub(in super::super) fn method() {}
}

pub(in super::super) use self::Holder as Alias;

mod inner {
    pub(in super::super::super) fn inline_item() {}
}
