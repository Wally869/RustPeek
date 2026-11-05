pub mod named {
    pub struct Real;
}

mod user {
    use super::named::Nope;

    fn call() {
        let _ = super::nomod::make();
    }

    fn typed(_: crate::inline_use::nomod::T) {}
}
