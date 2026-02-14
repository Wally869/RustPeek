// Good deep import
use crate::db::connection::Connection;
use crate::db::Pool;

// Bad deep import — module exists but item doesn't
use crate::db::connection::DatabaseHandle;

// Bad — module doesn't exist
use crate::db::migrations::Migration;

pub fn handler(_pool: &Pool) -> Connection {
    Connection { id: 0 }
}
