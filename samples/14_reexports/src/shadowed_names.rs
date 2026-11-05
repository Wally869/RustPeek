#![allow(dead_code)]

pub mod tables {
    pub const T: &[u8] = &[];
    pub const N: usize = 1;
    pub const INF: usize = 2;
    pub type Range = (u32, u32);
    pub type Result<T> = core::result::Result<T, u8>;
    pub struct Option;
    pub struct V;
    pub struct E;
    pub struct I;
    pub struct Span;
    pub struct TokenStream;
}

mod generics {
    type Res<T> = core::result::Result<T, u8>;

    fn spec<T>(r: Res<T>) -> Res<T> {
        r
    }

    struct Holder<V, const N: usize> {
        items: [V; N],
    }

    impl<V, const N: usize> Holder<V, N> {
        fn first<E>(&self, _e: E) -> Option<&V> {
            self.items.first()
        }
    }

    trait Maker<I> {
        fn make<E>(input: I) -> Option<E>;
    }

    enum Either<V, E> {
        Left(V),
        Right(E),
    }

    fn prelude(r: Result<u8, u8>, o: Option<u8>) -> Result<u8, u8> {
        let _ = o;
        r
    }
}

mod core_import {
    use core::ops::Range;

    fn span(r: Range<usize>) -> usize {
        r.start
    }
}

mod external_import {
    use proc_macro2::{Span, TokenStream};

    fn call_site() -> (Span, TokenStream) {
        (Span::call_site(), TokenStream::new())
    }
}

mod block_items {
    struct Buf<const N: usize>;

    fn sized() -> usize {
        const INF: usize = 3;
        struct V;
        let _buf: Buf<INF> = Buf;
        let _v: Option<V> = None;
        INF
    }
}
