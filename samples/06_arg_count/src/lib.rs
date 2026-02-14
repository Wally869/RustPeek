pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn greet(name: &str) {
    println!("Hello, {name}!");
}

pub fn no_args() -> i32 {
    42
}

// E0061: Wrong number of arguments
fn wrong_counts() {
    add(1, 2, 3);           // expects 2, got 3
    add(1);                  // expects 2, got 1
    greet("a", "b");         // expects 1, got 2
    no_args(42);             // expects 0, got 1
}

// Correct calls — should be silent
fn correct_calls() {
    add(1, 2);
    greet("world");
    no_args();
}
