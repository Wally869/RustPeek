// `#[cfg_attr(.., path = ..)]` on a `mod`: the tool cannot tell which file is loaded.
#[cfg_attr(feature = "s", path = "cfg_attr_path/b.rs")]
#[cfg_attr(not(feature = "s"), path = "cfg_attr_path/a.rs")]
pub mod lazy;

pub fn from_lazy() -> u32 {
    lazy::value()
}
