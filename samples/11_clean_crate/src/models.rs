pub struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
}

pub enum Role {
    Admin,
    Editor,
    Viewer,
}

impl User {
    pub fn new(id: u64, name: String, email: String) -> Self {
        User { id, name, email }
    }
}
