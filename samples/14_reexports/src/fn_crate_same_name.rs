// A fn is in the value namespace: it does not hide the crate `std` (type namespace).
pub fn std() {}

pub fn probe() -> usize {
    std::mem::size_of::<u8>()
}
