pub struct Config {
    pub host: String,
    pub port: u16,
    pub debug: bool,
}

// E0063: Missing required fields
fn missing_fields() {
    let _c = Config {
        host: "localhost".to_string(),
        // missing port and debug
    };
}

// E0609: Nonexistent field
fn wrong_field() {
    let _c = Config {
        host: "localhost".to_string(),
        port: 8080,
        debug: true,
        timeout: 30, // doesn't exist
    };
}

// Correct usage — should be silent
fn correct() {
    let _c = Config {
        host: "localhost".to_string(),
        port: 8080,
        debug: false,
    };
}

// Using .. rest syntax — should NOT flag missing fields
fn with_default(base: Config) {
    let _c = Config {
        host: "0.0.0.0".to_string(),
        ..base
    };
}
