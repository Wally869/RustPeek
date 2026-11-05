pub mod payload {
    pub struct Packet;
    pub fn send() {}
}

pub mod hub {
    pub use super::payload::*;
}

pub mod named {
    pub use super::payload::Packet as Frame;
    pub use super::payload::send;
}

pub mod empty {}

pub mod other {
    pub struct Unrelated;
}

mod user {
    use super::hub::Packet;
    use super::hub::send;
    use super::named::Frame;
    use super::empty;
    use super::empty::*;
    use crate::inline_mods::hub::*;

    struct Local;

    fn helper() {}

    fn run() {
        let _a: Packet = Packet;
        let _b: Frame = Frame;
        let _c: Local = Local;
        send();
        helper();
    }

    mod deeper {
        use super::Local;
        use self::Inner as Alias;
        use super::super::named::Frame;

        struct Inner;

        fn nested() {
            let _a: Local = Local;
            let _b: Alias = Inner;
            let _c: Frame = Frame;
        }
    }
}
