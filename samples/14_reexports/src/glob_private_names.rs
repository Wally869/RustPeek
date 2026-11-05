mod inner {
    pub struct Pub;
    pub struct PubT {}
}

mod pub_through_glob {
    use super::inner::*;

    fn value() {
        let _ = Pub;
        let _t: PubT = todo!();
    }
}

mod parent {
    struct Hidden;
    struct HiddenT {}

    mod child {
        use super::*;

        fn value() {
            let _ = Hidden;
            let _t: HiddenT = todo!();
        }
    }
}
