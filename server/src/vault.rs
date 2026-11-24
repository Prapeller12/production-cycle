// Historical HTTP build uses its existing SQLite format.
use rusqlite::{Connection, DatabaseName, OpenFlags, backup::Progress};
use std::path::Path;
pub fn migrate_plain(_: &Path)->Result<(),String>{Ok(())}
pub fn open(path:&Path,flags:OpenFlags)->Result<Connection,String>{Connection::open_with_flags(path,flags).map_err(|e|e.to_string())}
pub fn read(path:&Path)->Result<Connection,String>{open(path,OpenFlags::SQLITE_OPEN_READ_ONLY)}
pub fn backup(c:&Connection,path:&Path)->Result<(),String>{c.backup(DatabaseName::Main,path,None).map_err(|e|e.to_string())}
pub fn restore(c:&mut Connection,path:&Path)->Result<(),String>{c.restore(DatabaseName::Main,path,None::<fn(Progress)>).map_err(|e|e.to_string())}
