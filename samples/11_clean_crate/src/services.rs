use crate::models::{User, Role};

pub fn create_admin(id: u64, name: String, email: String) -> User {
    let _role = Role::Admin;
    User::new(id, name, email)
}

pub fn greet(user: &User) -> String {
    format!("Hello, {}!", user.name)
}
