pub struct PublicThing {
    pub name: String,
}

struct PrivateStruct {
    data: Vec<u8>,
}

pub(crate) fn crate_visible() -> i32 {
    42
}

fn private_fn() -> bool {
    true
}
