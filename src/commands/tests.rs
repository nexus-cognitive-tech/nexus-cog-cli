use super::*;
use crate::ctx::Ctx;
use tempfile::tempdir;

#[test]
fn palace_rooms_and_summary() {
    let dir = tempdir().unwrap();
    let db = dir.path().join("p.db");
    let ctx = Ctx::open(db, "test").unwrap();
    rooms(&ctx).unwrap();
    summary(&ctx).unwrap();
}
