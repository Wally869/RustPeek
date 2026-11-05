// T3: a path call resolves through its own path, not by its last segment.
// T4: associated items and trait items of an enum are not variants.

fn f(_a: u8) {}

fn lit() {}

pub struct S;

impl S {
    pub fn f(_a: u8, _b: u8) {}
    pub fn lit(_a: u8) -> S {
        S
    }
}

pub type Alias = S;

mod scalar {
    pub fn f(_a: u8, _b: u8) {}
}

pub trait Make {
    fn make(a: u8, b: u8) -> Self;
}

impl Make for S {
    fn make(_a: u8, _b: u8) -> S {
        S
    }
}

fn drop(_a: u8, _b: u8) {}

fn calls() {
    f(1);
    lit();
    S::f(1, 2);
    Alias::f(1, 2);
    scalar::f(1, 2);
    self::scalar::f(1, 2);
    let _ = S::lit(1);
    let _: S = Make::make(1, 2);
    let _ = <S as Make>::make(1, 2);
    std::mem::drop(1u8);
    drop(1, 2);
}

#[derive(Default, Clone, Copy)]
pub enum Op {
    #[default]
    Exact,
    Less,
}

impl Op {
    pub const DEFAULT: Op = Op::Exact;

    pub fn make() -> Op {
        Op::DEFAULT
    }
}

impl From<u8> for Op {
    fn from(_: u8) -> Op {
        Op::Less
    }
}

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

#[derive(Default)]
pub struct Plain {
    pub n: u8,
}

pub struct Handmade;

impl Default for Handmade {
    fn default() -> Handmade {
        Handmade
    }
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

pub fn assoc_items() {
    let _ = Tagged::build();
    let _ = Tagged::LIMIT;
    let _ = Tagged::describe_one();
    let _ = Tagged::describe_two();
    let _ = Tagged::everywhere();
    let _ = Plain::default();
    let _ = Handmade::default();
    let _ = Handmade::from(Handmade);
}

pub type OpAlias = Op;

pub fn assoc() -> Op {
    let _ = Op::from(1);
    let _ = Op::default();
    let _ = Op::make();
    let _ = OpAlias::make();
    let _ = Op::Less;
    Op::DEFAULT
}
