struct Hidden;
mod hidden_mod {}
pub(crate) struct CrateOnly;
fn secret() {}
struct Braced { x: u8 }
struct Tup(u8);
enum En { A }
trait Tr {}
type Al = u8;
const K: u8 = 0;
static SK: u8 = 0;

pub mod outer {
    pub use super::Hidden;
    pub use super::hidden_mod;
    pub use super::CrateOnly;
    pub(crate) use super::secret;
    pub use super::Braced;
    pub use super::Tup;
    pub use super::En;
    pub use super::Tr;
    pub use super::Al;
    pub use super::K;
    pub use super::SK;
}
