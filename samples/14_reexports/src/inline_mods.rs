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
    use crate::inline_mods::hub::*;}
