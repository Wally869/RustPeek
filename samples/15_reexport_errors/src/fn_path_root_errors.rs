// A fn is in the value namespace: it cannot start a path (rustc E0433 on each line).
fn func() {}

pub fn probe() {
    let _ = func::x;
    func::y();
    let _z: func::T;
}
