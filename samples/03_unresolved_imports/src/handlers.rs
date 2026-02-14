// Good import — should be silent
use crate::models::User;

// E0432: Item doesn't exist in that module
use crate::models::Comment;

// E0433: Module doesn't exist at all
use crate::services::AuthService;

// E0432: Typo in item name
use crate::models::Postt;

pub fn handle_user(_u: User) {}
