pub struct Thing;

pub mod items {
    pub struct Item;
    pub struct Other;
    pub struct Third;
    pub fn make() -> Item {
        Item
    }
}

pub fn named() -> usize {
    use crate::fn_uses::items::Item;
    let _value = Item;
    let _typed: Option<Item> = None;
    0
}

pub fn renamed() {
    use crate::fn_uses::items::Item as Renamed;
    let _value: Renamed = Renamed;
}

pub fn grouped() {
    use crate::fn_uses::items::{Other, Third};
    let _pair: (Other, Third) = (Other, Third);
}

pub fn globbed() {
    let _early: Option<Other> = None;
    use crate::fn_uses::items::*;
    let _value: Item = make();
}

pub fn self_form() {
    use self::items::Other;
    let _value: Other = Other;
}

pub fn nested_block() {
    {
        use crate::fn_uses::items::Third;
        let _value: Third = Third;
        {
            let _inner: Third = Third;
        }
    }
}

pub mod child {
    pub fn uses_super() {
        use super::Thing;
        let _value: Thing = Thing;
    }

    pub fn uses_super_items() {
        use super::items::{self, Item};
        let _value: Item = items::make();
    }
}
