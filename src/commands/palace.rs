//! Palace subcommands.

use anyhow::Result;

use crate::ctx::Ctx;

pub fn rooms(ctx: &Ctx) -> Result<()> {
    for r in ctx.palace.rooms() {
        println!("{} [{}] ({} items)", r.id, r.name, r.items.len());
    }
    Ok(())
}

pub fn summary(ctx: &Ctx) -> Result<()> {
    let s = ctx.palace.summary();
    println!(
        "{} rooms, {} items, {} connections",
        s.total_rooms, s.total_items, s.total_connections
    );
    Ok(())
}

pub fn add_item(ctx: &Ctx, room: &str, key: &str, value: &str, confidence: f32) -> Result<()> {
    use nexus_cog_core::palace::MemoryItem;
    ctx.palace
        .add_item(room, MemoryItem::new(key, value, confidence))?;
    ctx.save()?;
    println!("added `{}` to room `{}`", key, room);
    Ok(())
}
