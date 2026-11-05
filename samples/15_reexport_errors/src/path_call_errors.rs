// T3: a wrong arity through a module path is still E0061.
// T4: a missing variant of a real enum is still E0599.

mod scalar {
    pub fn imul(_a: u8, _b: u8) {}
}

fn imul(_a: u8, _b: u8, _c: u8) {}

fn calls() {
    scalar::imul(1, 2, 3);
    self::scalar::imul(1);
    imul(1, 2, 3);
}

pub enum Kind {
    Exact,
}

fn variant() {
    let _ = Kind::Exact;
    let _ = Kind::Missing;
    let _ = Kind::missing_fn();
    let _ = Kind::MISSING;
}

pub struct Figure {
    pub n: u8,
}

impl Figure {
    pub fn new() -> Figure {
        Figure { n: 0 }
    }
}

fn figure() {
    let _ = Figure::new();
    let _ = Figure::missing_fn();
    let _ = Figure::MISSING;
}
