use crate::db::Pool;

pub struct Connection {
    pub id: u32,
}

pub fn connect(_pool: &Pool) -> Connection {
    Connection { id: 1 }
}
