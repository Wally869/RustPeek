mod inner {
    pub struct Pub;
    struct Priv;
    struct PrivT {}
}

mod module_level {
    use super::inner::*;

    fn value() {
        let _ = Priv;
    }

    fn typ() {
        let _t: PrivT = todo!();
    }
}

mod block_level {
    fn value() {
        use super::inner::*;
        let _ = Priv;
    }

    fn typ() {
        use super::inner::*;
        let _t: PrivT = todo!();
    }
}
