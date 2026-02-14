// No imports at all — all of these exist in-crate but aren't imported.
// Rustpeek should suggest where they live.

// Using a type from another module without importing it
fn make_user() -> User {
    User { id: 1, name: "Alice".to_string() }
}

// Using an enum from another module without importing it
fn get_role() -> Role {
    Role::Admin
}

// A type that doesn't exist anywhere — should be SILENT (could be external)
fn external_thing() -> SomeExternalType {
    todo!()
}
