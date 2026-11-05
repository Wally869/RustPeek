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

pub type OpAlias = Op;

pub fn assoc() -> Op {
    let _ = Op::from(1);
    let _ = Op::default();
    let _ = Op::make();
    let _ = OpAlias::make();
    let _ = Op::Less;
    Op::DEFAULT
}
