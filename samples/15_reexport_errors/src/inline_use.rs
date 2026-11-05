pub mod named {
    pub struct Real;
}

mod user {
    use super::named::Nope;
}
