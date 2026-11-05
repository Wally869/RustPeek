extern crate core as kore;
pub use core::ops::*;
use core::fmt;
use std::io as imp;
pub use self::imp::{Error, ErrorKind, Result};
use crate::alloc::alloc::{alloc, dealloc, Layout};
use self::fmt::Write;

mod inner {
    use super::Range;
    use super::fmt::Debug;
    use super::Error;
    use crate::extern_paths::{Range as R2, ErrorKind};
    use crate::extern_paths::imp::Write as IoWrite;
    use crate::extern_paths::kore::mem::swap;
    use crate::extern_paths::{Error as E2, RangeFull};

    pub fn through_paths(a: &mut u8, b: &mut u8) {
        kore::mem::drop(1u8);
        crate::extern_paths::kore::mem::drop(2u8);
        super::kore::mem::drop(4u8);
        super::kore::mem::swap(a, b);
        let _: Option<super::imp::Error> = None;
        let _: Option<crate::extern_paths::fmt::Error> = None;
        let _: Option<crate::alloc::alloc::Layout> = None;
        let _ = (Range { start: 0, end: 1 }, R2 { start: 0, end: 1 }, RangeFull);
    }
}

pub fn uses_fmt() {
    use self::fmt::Write as W;
    self::kore::mem::drop(5u8);
    let _ = (alloc,dealloc, Layout::new::<u8>());
}
