pub enum Color {
    Red,
    Green,
    Blue,
    Custom(u8, u8, u8),
}

pub enum Status {
    Active,
    Inactive,
    Suspended,
}

impl Status {
    pub fn is_active(&self) -> bool {
        matches!(self, Status::Active)
    }
}

// E0599: Variant doesn't exist
fn bad_variant() {
    let _c = Color::Yellow;     // no such variant
    let _s = Status::Deleted;   // no such variant
}

// Should be silent — correct variants
fn good_variants() {
    let _c = Color::Red;
    let _s = Status::Active;
}

// Should be silent — this is an associated function, not a variant
fn assoc_fn() {
    let _active = Status::is_active;
}
