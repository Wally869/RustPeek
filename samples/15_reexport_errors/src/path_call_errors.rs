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

// Round 9 review: valid `Type::name` cases in a crate with no item-level macro call
// (sample 14 has one, so the E0599 check is off there). None of these is reported.
pub trait Describe {
    fn describe_one() -> u8;
    fn describe_two() -> u8 {
        2
    }
}

pub trait Everywhere {
    fn everywhere() -> u8 {
        0
    }
}

impl<T> Everywhere for T {}

#[derive(Default, Clone)]
pub struct Plain {
    pub n: u8,
}

pub struct Tagged;

impl Tagged {
    pub const LIMIT: u8 = 3;
    pub fn build() -> Tagged {
        Tagged
    }
}

impl Describe for Tagged {
    fn describe_one() -> u8 {
        1
    }
}

mod real {
    pub struct Cell(pub u8);
    impl Cell {
        pub fn new() -> Cell {
            Cell(0)
        }
    }
}
use self::real::Cell as Renamed;

fn valid_assoc() {
    let _ = Tagged::build();
    let _ = Tagged::LIMIT;
    let _ = Tagged::describe_one();
    let _ = Tagged::describe_two();
    let _ = Tagged::everywhere();
    let _ = Plain::default();
    let _ = Plain::clone(&Plain { n: 1 });
    let _ = Plain::from(Plain { n: 2 });
    let _ = Renamed::new();
}

// Reported (E0599): a local trait impl lacks the name; a local trait method on a type
// that does not implement the trait.
pub struct Lacking;

impl Describe for Lacking {
    fn describe_one() -> u8 {
        1
    }
}

pub struct NoTrait;

fn missing_assoc() {
    let _ = Lacking::describe_three();
    let _ = NoTrait::describe_one();
}

// Not reported (a limit; rustc gives E0599): a type with an impl of a non-local trait
// may get the name from it, and an alias is not checked.
pub struct Shown;

impl std::fmt::Display for Shown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("shown")
    }
}

pub type KindAlias = Kind;

fn skipped_assoc() {
    let _ = Shown::missing_fn();
    let _ = KindAlias::missing_fn();
}
