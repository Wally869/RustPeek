pub struct AstNode {
    pub kind: String,
}

pub fn parse_input(input: &str) -> AstNode {
    AstNode { kind: input.to_string() }
}
