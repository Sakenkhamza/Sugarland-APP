use rusqlite::{Connection, Result};

fn main() -> Result<()> {
    let conn = Connection::open("sugarland.db")?;
    
    // Check if column exists, if not add it
    let mut stmt = conn.prepare("PRAGMA table_info(auctions)")?;
    let mut rows = stmt.query([])?;
    let mut has_vendor_id = false;
    
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == "vendor_id" {
            has_vendor_id = true;
            break;
        }
    }
    
    if !has_vendor_id {
        println!("Adding vendor_id column to auctions table...");
        conn.execute("ALTER TABLE auctions ADD COLUMN vendor_id TEXT", [])?;
        println!("Successfully added vendor_id column.");
    } else {
        println!("vendor_id column already exists.");
    }
    
    Ok(())
}
