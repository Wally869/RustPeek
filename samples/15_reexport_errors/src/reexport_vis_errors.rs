struct Hidden;
mod hidden_mod {}
pub(crate) struct CrateOnly;
fn secret() {}

pub mod outer {
    pub use super::Hidden;
    pub use super::hidden_mod;
    pub use super::CrateOnly;
    pub(crate) use super::secret;
}
