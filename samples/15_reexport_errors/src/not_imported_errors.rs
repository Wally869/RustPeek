pub mod defs {
    pub const LIMIT: u32 = 1;
    pub static COUNTER: u32 = 2;
    pub struct Marker;
    pub struct Pair(pub u32, pub u32);
    pub struct Shape {
        pub side: u32,
    }
}

fn use_const() -> u32 {
    LIMIT
}

fn use_static() -> u32 {
    COUNTER
}

fn use_unit_struct() {
    let _m = Marker;
}

fn use_tuple_struct() {
    let _p = Pair(1, 2);
}

fn use_type(_s: Shape) {}
