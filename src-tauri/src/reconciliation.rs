use crate::db::Database;
use crate::csv_parser;
use rusqlite::Result;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct ReconciliationResult {
    pub sold_count: i32,
    pub buyback_count: i32,
    pub total_revenue: f64,
    pub total_profit: f64,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ProfitLossReport {
    pub total_revenue: f64,
    pub total_cogs: f64,
    pub gross_profit: f64,
    pub total_expenses: f64,
    pub net_profit: f64,
    pub margin_percent: f64,
    pub sold_items: i32,
}

pub struct ReconciliationManager;

impl ReconciliationManager {
    pub fn reconcile_hibid_results(
        db: &Database,
        auction_id: &str,
        file_path: &str,
    ) -> Result<ReconciliationResult, String> {
        // 1. Parse Results CSV
        let results = csv_parser::parse_hibid_results(file_path)
            .map_err(|e| e.to_string())?;

        let mut sold_count = 0;
        let mut buyback_count = 0;
        let mut total_revenue = 0.0;
        let mut total_profit = 0.0;
        let mut errors = Vec::new();

        let tx = db.conn.unchecked_transaction().map_err(|e| e.to_string())?;

        // Load buyback bidder ID from settings
        let buyback_bidder_id: String = db.conn.query_row(
            "SELECT value FROM settings WHERE key = 'ron_larsson_bidder_id'",
            [],
            |row| row.get(0),
        ).unwrap_or_else(|_| {
            log::warn!("Ron Larsson ID not found in settings, using default '5046'");
            "5046".to_string()
        });

        // Load commission rate from settings
        let commission_rate: f64 = db.conn.query_row(
            "SELECT value FROM settings WHERE key = 'default_commission_rate'",
            [],
            |row| {
                let val: String = row.get(0)?;
                Ok(val.parse::<f64>().unwrap_or(0.15))
            },
        ).unwrap_or(0.15);

        for row in results {
            // Check if buyback
            let is_buyback = row.bidder_id == buyback_bidder_id;
            let status = if is_buyback { "Buyback" } else { "Sold" };
            
            let high_bid = csv_parser::clean_price(&row.high_bid);

            // Update inventory item status
            let updated = tx.execute(
                "UPDATE inventory_items 
                 SET current_status = ?1, sold_at = CURRENT_TIMESTAMP 
                 WHERE lot_number = ?2 AND auction_id = ?3 AND current_status = 'Listed'",
                rusqlite::params![status, row.lot_number, auction_id],
            ).map_err(|e| e.to_string())?;

            if updated == 0 {
                errors.push(format!("Lot {}: Item not found or not listed in this auction", row.lot_number));
                continue;
            }

            // Get item ID to insert result
            let item_id: String = tx.query_row(
                "SELECT id FROM inventory_items WHERE lot_number = ?1 AND auction_id = ?2",
                rusqlite::params![row.lot_number, auction_id],
                |r| r.get(0),
            ).unwrap_or_default();

            // Calculate basics
            let cost: f64 = tx.query_row(
                "SELECT cost_price FROM inventory_items WHERE id = ?1",
                rusqlite::params![item_id],
                |r| r.get(0),
            ).unwrap_or(0.0);

            let commission = if is_buyback { 0.0 } else { high_bid * commission_rate };
            let net_profit = if is_buyback {
                0.0 // Buyback is neutral/loss usually, handled separately
            } else {
                high_bid - cost - commission
            };

            // Insert auction result
            tx.execute(
                "INSERT INTO auction_results 
                 (id, auction_id, item_id, winning_bidder, bidder_id, high_bid, max_bid, 
                  is_buyback, commission_rate, commission_amount, net_profit)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    auction_id,
                    item_id,
                    row.winning_bidder,
                    row.bidder_id,
                    high_bid,
                    csv_parser::clean_price(&row.max_bid.unwrap_or_default()),
                    is_buyback,
                    commission_rate,
                    commission,
                    net_profit
                ],
            ).map_err(|e| e.to_string())?;

            if is_buyback {
                buyback_count += 1;
            } else {
                sold_count += 1;
                total_revenue += high_bid;
                total_profit += net_profit;
            }
        }

        tx.commit().map_err(|e| e.to_string())?;

        // Update auction status to Completed
        db.conn.execute(
            "UPDATE auctions SET status = 'Completed' WHERE id = ?1",
            rusqlite::params![auction_id],
        ).map_err(|e| e.to_string())?;

        Ok(ReconciliationResult {
            sold_count,
            buyback_count,
            total_revenue,
            total_profit,
            errors,
        })
    }

    pub fn generate_pl_report(db: &Database) -> Result<ProfitLossReport, String> {
        let sql = "
            SELECT 
                COUNT(*) as sold_items,
                COALESCE(SUM(high_bid), 0) as revenue,
                COALESCE(SUM(i.cost_price), 0) as cogs,
                COALESCE(SUM(commission_amount), 0) as expenses,
                COALESCE(SUM(net_profit), 0) as net_profit
            FROM auction_results ar
            JOIN inventory_items i ON ar.item_id = i.id
            WHERE ar.is_buyback = FALSE
        ";

        let (sold_items, revenue, cogs, expenses, net_profit): (i32, f64, f64, f64, f64) = 
            db.conn.query_row(sql, [], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            }).map_err(|e| e.to_string())?;

        let gross_profit = revenue - cogs;
        let margin_percent = if revenue > 0.0 { (net_profit / revenue) * 100.0 } else { 0.0 };

        Ok(ProfitLossReport {
            sold_items,
            total_revenue: revenue,
            total_cogs: cogs,
            gross_profit,
            total_expenses: expenses,
            net_profit,
            margin_percent,
        })
    }
}

// Tauri Commands

#[tauri::command]
pub fn reconcile_auction(
    auction_id: String,
    file_path: String,
    state: State<crate::AppState>,
) -> Result<ReconciliationResult, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    ReconciliationManager::reconcile_hibid_results(&db, &auction_id, &file_path)
}

#[tauri::command]
pub fn get_pl_report(state: State<crate::AppState>) -> Result<ProfitLossReport, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    ReconciliationManager::generate_pl_report(&db)
}
