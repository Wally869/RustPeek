// Forgot to import AstNode — it exists in crate::parser
// Should get a suggestion, not an error (since it COULD be external,
// but we know it's in-crate so we suggest it)
fn process(node: AstNode) -> String {
    node.kind
}

// Typo in import path — item doesn't exist but similar one does
use crate::parser::Astnode;  // should suggest AstNode
