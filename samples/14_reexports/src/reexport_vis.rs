pub struct Open;
pub mod open_mod {}
pub(crate) struct CrateOnly;
struct Hidden;
pub(crate) struct CBraced { x: u8 }
pub(crate) struct CTup(u8);
pub(crate) enum CEn { A }
pub(crate) trait CTr {}
pub(crate) type CAl = u8;
pub(crate) const CK: u8 = 0;
pub(crate) static CSK: u8 = 0;
pub struct PBraced { x: u8 }
pub struct PTup(u8);
pub enum PEn { A }
pub trait PTr {}
pub type PAl = u8;
pub const PK: u8 = 0;
pub static PSK: u8 = 0;

pub mod outer {
    pub use super::Open;
    pub use super::open_mod;
    pub(crate) use super::CrateOnly;
    use super::Hidden;
    pub(crate) use super::CBraced;
    pub(crate) use super::CTup;
    pub(crate) use super::CEn;
    pub(crate) use super::CTr;
    pub(crate) use super::CAl;
    pub(crate) use super::CK;
    pub(crate) use super::CSK;
    pub use super::PBraced;
    pub use super::PTup;
    pub use super::PEn;
    pub use super::PTr;
    pub use super::PAl;
    pub use super::PK;
    pub use super::PSK;
}
