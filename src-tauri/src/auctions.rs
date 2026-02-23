use crate::db::Database;
use crate::hibid;
use rusqlite::Result;
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Auction {
    pub id: String,
    pub hibid_auction_id: Option<String>,
    pub name: String,
    pub vendor_id: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub status: String,
    pub total_lots: i32,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateAuctionRequest {
    pub name: String,
    pub vendor_id: Option<String>,
    pub hibid_auction_id: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateVendorRequest {
    pub cost_coefficient: f64,
    pub min_price_margin: f64,
}

pub struct AuctionManager;

impl AuctionManager {
    pub fn create_auction(db: &Database, req: CreateAuctionRequest) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        
        db.conn.execute(
            "INSERT INTO auctions (id, hibid_auction_id, name, vendor_id, start_date, end_date, status, total_lots)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'Draft', 0)",
            rusqlite::params![
                id,
                req.hibid_auction_id,
                req.name,
                req.vendor_id,
                req.start_date,
                req.end_date
            ],
        )?;

        Ok(id)
    }

    pub fn list_auctions(db: &Database) -> Result<Vec<Auction>> {
        let mut stmt = db.conn.prepare(
            "SELECT id, hibid_auction_id, name, vendor_id, start_date, end_date, status, total_lots, created_at
             FROM auctions ORDER BY created_at DESC",
        )?;

        let auctions = stmt.query_map([], |row| {
            Ok(Auction {
                id: row.get(0)?,
                hibid_auction_id: row.get(1)?,
                name: row.get(2)?,
                vendor_id: row.get(3)?,
                start_date: row.get(4)?,
                end_date: row.get(5)?,
                status: row.get(6)?,
                total_lots: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;

        Ok(auctions)
    }

    pub fn assign_items_to_auction(db: &Database, auction_id: &str, item_ids: Vec<String>) -> Result<usize> {
        let mut count = 0;
        let tx = db.conn.unchecked_transaction()?;

        for item_id in item_ids {
            tx.execute(
                "UPDATE inventory_items SET auction_id = ?1, current_status = 'Listed', listed_at = CURRENT_TIMESTAMP WHERE id = ?2",
                rusqlite::params![auction_id, item_id],
            )?;
            count += 1;
        }
        
        // Update auction total_lots count
        tx.execute(
            "UPDATE auctions SET total_lots = (SELECT COUNT(*) FROM inventory_items WHERE auction_id = ?1) WHERE id = ?1",
            rusqlite::params![auction_id],
        )?;

        tx.commit()?;
        Ok(count)
    }

    pub fn get_auction_by_id(db: &Database, auction_id: &str) -> Result<Auction> {
        db.conn.query_row(
            "SELECT id, hibid_auction_id, name, vendor_id, start_date, end_date, status, total_lots, created_at
             FROM auctions WHERE id = ?1",
            rusqlite::params![auction_id],
            |row| {
                Ok(Auction {
                    id: row.get(0)?,
                    hibid_auction_id: row.get(1)?,
                    name: row.get(2)?,
                    vendor_id: row.get(3)?,
                    start_date: row.get(4)?,
                    end_date: row.get(5)?,
                    status: row.get(6)?,
                    total_lots: row.get(7)?,
                    created_at: row.get(8)?,
                })
            }
        )
    }

    pub fn update_auction_status(db: &Database, auction_id: &str, status: &str) -> Result<()> {
        db.conn.execute(
            "UPDATE auctions SET status = ?1 WHERE id = ?2",
            rusqlite::params![status, auction_id],
        )?;
        Ok(())
    }

    pub fn update_vendor(db: &Database, vendor_id: &str, data: &UpdateVendorRequest) -> Result<()> {
        db.conn.execute(
            "UPDATE vendors SET cost_coefficient = ?1, min_price_margin = ?2 WHERE id = ?3",
            rusqlite::params![data.cost_coefficient, data.min_price_margin, vendor_id],
        )?;
        Ok(())
    }
}

// Tauri Commands

#[tauri::command]
pub fn create_auction(
    req: CreateAuctionRequest,
    state: State<crate::AppState>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    AuctionManager::create_auction(&db, req).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_auctions(state: State<crate::AppState>) -> Result<Vec<Auction>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    AuctionManager::list_auctions(&db).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn assign_items(
    auction_id: String,
    item_ids: Vec<String>,
    state: State<crate::AppState>,
) -> Result<usize, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    AuctionManager::assign_items_to_auction(&db, &auction_id, item_ids).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn export_auction_csv(
    auction_id: String,
    file_path: String,
    state: State<crate::AppState>,
) -> Result<usize, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    
    // 1. Get items for this auction
    let items = db.get_inventory_items(Some("Listed"))
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|item| item.auction_id.as_deref() == Some(&auction_id))
        .collect::<Vec<_>>();

    if items.is_empty() {
        return Err("No items found for this auction".to_string());
    }

    // 2. Export to CSV using hibid module
    hibid::export_to_hibid_csv(&items, &file_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_auction_by_id(
    auction_id: String,
    state: State<crate::AppState>,
) -> Result<Auction, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    AuctionManager::get_auction_by_id(&db, &auction_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_auction_status(
    auction_id: String,
    status: String,
    state: State<crate::AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    AuctionManager::update_auction_status(&db, &auction_id, &status).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_vendor(
    vendor_id: String,
    data: UpdateVendorRequest,
    state: State<crate::AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    AuctionManager::update_vendor(&db, &vendor_id, &data).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn unassign_item(
    item_id: String,
    state: State<crate::AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    
    // Get auction_id before reset
    let auction_id: Option<String> = db.conn.query_row(
        "SELECT auction_id FROM inventory_items WHERE id = ?1",
        rusqlite::params![item_id],
        |r| r.get(0),
    ).unwrap_or(None);

    // Reset status and auction_id
    db.conn.execute(
        "UPDATE inventory_items SET current_status = 'InStock', auction_id = NULL, listed_at = NULL WHERE id = ?1",
        rusqlite::params![item_id],
    ).map_err(|e| e.to_string())?;

    // Update auction total_lots count
    if let Some(auc_id) = auction_id {
        db.conn.execute(
            "UPDATE auctions SET total_lots = (
                SELECT COUNT(*) FROM inventory_items WHERE auction_id = ?1
             ) WHERE id = ?1",
            rusqlite::params![auc_id],
        ).map_err(|e| e.to_string())?;
    }

    Ok(())
}
