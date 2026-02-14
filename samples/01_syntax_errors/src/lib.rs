// Pass 1 should catch these and stop before indexing

pub fn broken_syntax( {
    // missing closing paren
}

pub struct Incomplete {
    name: String,
    // missing closing brace
