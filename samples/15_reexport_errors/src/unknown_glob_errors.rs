use missing::*;

mod nested {
    mod deeper {
        use missing::*;
    }
}

fn in_body() {
    use missing::*;
}
