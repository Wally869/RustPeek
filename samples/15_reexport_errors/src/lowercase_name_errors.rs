mod inner {
    pub fn open_fn() {}
    fn hidden_fn() {}
    const HIDDEN_CONST: u32 = 1;
}

mod elsewhere {
    pub fn only_here() {}
}

mod behind_glob {
    use super::inner::*;

    fn private_fn() {
        hidden_fn();
    }

    fn private_const() -> u32 {
        HIDDEN_CONST
    }

    fn unknown() {
        nowhere();
    }

    fn not_imported() {
        only_here();
    }

    fn fine() {
        open_fn();
    }
}

mod first_segment {
    fn missing_fn() {
        missing::f();
    }

    fn missing_const() -> u32 {
        missing::X
    }
}
