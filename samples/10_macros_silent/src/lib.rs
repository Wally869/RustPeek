// Macros generate code the indexer can't see.
// Rustpeek should stay silent about anything that could come from macros.

macro_rules! make_struct {
    ($name:ident) => {
        pub struct $name {
            pub value: i32,
        }
    };
}

make_struct!(Generated);

// Using the macro-generated struct — we can't know about it, stay quiet
fn use_generated() {
    let _g = Generated { value: 42 };
}

// derive macros generate methods we can't see
#[derive(Debug, Clone)]
pub struct Cloneable {
    pub data: String,
}

fn clone_it(c: &Cloneable) -> Cloneable {
    c.clone()
}
