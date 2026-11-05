pub mod items {
    pub struct Item;
}

pub fn missing_item() {
    use crate::fn_use_errors::items::Missing;
}

pub fn missing_module() {
    use crate::nomod::Thing;
}
