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
}
