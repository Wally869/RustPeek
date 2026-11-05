use std::fmt;

pub mod util {
    pub fn helper() -> u32 { 1 }
    pub const LIMIT: u32 = 2;
    pub static COUNTER: u32 = 3;
    pub mod deep {
        pub fn deeper() -> u32 { 4 }
    }
}

mod ffi_like {
    pub use std::cmp::max;
    pub use std::mem::swap as exchange;
}

mod user {
    use super::util::*;
    use super::ffi_like::*;

    fn values() -> u32 {
        let a = helper() + LIMIT + COUNTER;
        let b = deep::deeper();
        let c = max(a, b);
        let mut x = 1;
        let mut y = 2;
        exchange(&mut x, &mut y);
        c + x + y
    }
}

struct Holder {
    value: u32,
}

impl Holder {
    fn new(value: u32) -> Self { Holder { value } }

    fn make() -> Self { Self::new(1) }

    fn read(&self, extra: u32) -> u32 {
        let Holder { value } = self;
        let total = *value + extra + self.value;
        total
    }

    fn write(&mut self, v: u32) { self.value = v; }
}

fn generic<T: Default + Clone>(input: T) -> T {
    let fresh = T::default();
    let _ = fresh;
    input.clone()
}

fn bindings(opt: Option<u32>, pair: (u32, u32), items: &[u32]) -> u32 {
    let Some(first) = opt else { return 0 };
    let (left, right) = pair;
    let [head, .., tail] = [1, 2, 3];
    let mut sum = first + left + right + head + tail;
    if let Some(inner) = opt {
        sum += inner;
    }
    let mut it = items.iter();
    while let Some(item) = it.next() {
        sum += item;
    }
    for (index, value) in items.iter().enumerate() {
        sum += index as u32 + value;
    }
    match opt {
        Some(n @ 1..=5) => sum += n,
        Some(ref other) if *other > 5 => sum += other,
        _ => {}
    }
    let closure = |a: u32, (b, c): (u32, u32)| a + b + c + sum;
    closure(1, (2, 3))
}

fn nested() -> u32 {
    fn inner(x: u32) -> u32 { x + 1 }
    const LOCAL: u32 = 2;
    static LOCAL_STATIC: u32 = 3;
    mod_in_fn::go() + inner(LOCAL) + LOCAL_STATIC
}

mod mod_in_fn {
    pub fn go() -> u32 { 0 }
}

fn primitives() -> u32 {
    let a = u32::MAX;
    let b = str::from_utf8(&[]).map(|s| s.len()).unwrap_or(0);
    let c = <u32 as Default>::default();
    let d = std::mem::size_of::<u32>();
    let e = core::cmp::max(1, 2);
    let f = alloc::vec::Vec::<u32>::new().len();
    let g = serde::de::IgnoredAny;
    let h = proc_macro2::Span::call_site();
    let _ = (a, b, c, d, e, f, g, h);
    drop(1);
    0
}

struct Show;

impl fmt::Display for Show {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "show")
    }
}

mod tests {
    use super::*;

    fn uses_parent() -> u32 {
        let s = Show.to_string();
        let _ = fmt::format(format_args!("{}", s));
        generic(1) + nested() + bindings(None, (1, 2), &[])
    }
}
