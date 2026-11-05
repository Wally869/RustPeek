// The suggested `use` must name the inline module: `fix` then leaves 0 errors.
mod shapes;

pub fn area(c: Circle) -> u32 {
    c.r * c.r * 3
}
