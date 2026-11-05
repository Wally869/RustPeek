pub struct Open;
pub mod open_mod {}
pub(crate) struct CrateOnly;
struct Hidden;

pub mod outer {
    pub use super::Open;
    pub use super::open_mod;
    pub(crate) use super::CrateOnly;
    use super::Hidden;
}
