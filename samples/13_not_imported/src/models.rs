pub struct User {
    pub id: u64,
    pub name: String,
}

pub struct Config {
    pub debug: bool,
}

pub enum Role {
    Admin,
    Editor,
    Viewer,
}
