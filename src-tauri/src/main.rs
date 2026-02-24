// Sugarland — Main Entry Point
// Tauri application for liquidation inventory management

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod csv_parser;
mod pricing;
mod hibid;
mod auctions;
mod reconciliation;
mod nlp;

use std::sync::Mutex;
use db::Database;
use serde::Serialize;

pub struct AppState {
    pub db: Mutex<Database>,
}

#[derive(Debug, Serialize)]
pub struct ManifestSummary {
    pub id: String,
    pub items_count: usize,
    pub total_retail: f64,
    pub total_cost: f64,
}

#[derive(Debug, Serialize)]
pub struct ReconciliationSummary {
    pub sold_count: i32,
    pub buyback_count: i32,
    pub total_revenue: f64,
    pub total_profit: f64,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct AuctionPnlRow {
    pub auction_id: String,
    pub auction_name: String,
    pub start_date: Option<String>,
    pub total_items: i64,
    pub sold_items: i64,
    pub buyback_items: i64,
    pub total_revenue: f64,
    pub total_cost: f64,
    pub total_commission: f64,
    pub net_profit: f64,
}

// ============================================================
// Tauri Commands
// ============================================================

#[tauri::command]
fn import_manifest(
    file_path: String,
    auction_id: Option<String>,
    state: tauri::State<AppState>,
) -> Result<ManifestSummary, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    // ... implementation same as before ...
    // Re-implemented for brevity or keep existing if using MultiReplace
    // Since this is ReplaceFileContent, I must be careful not to delete logic.
    // The previous prompt had the full implementation.
    // I will use the existing implementation logic from previous step 156.
    
    let pricing_engine = pricing::PricingEngine::new(&db.conn).map_err(|e| e.to_string())?;

    // 1. Parse CSV
    let rows = csv_parser::parse_bstock_csv(&file_path).map_err(|e| e.to_string())?;

    // 2. Create manifest
    let manifest_id = uuid::Uuid::new_v4().to_string();
    let filename = std::path::Path::new(&file_path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("unknown.csv");

    db.conn
        .execute(
            "INSERT INTO manifests (id, source_filename, items_count) VALUES (?1, ?2, ?3)",
            rusqlite::params![manifest_id, filename, rows.len()],
        )
        .map_err(|e| e.to_string())?;

    // 3. Process each row
    let mut total_retail = 0.0;
    let mut total_cost = 0.0;

    let nlp_extractor = nlp::EntityExtractor::new();

    for row in &rows {
        let retail_price = csv_parser::clean_price(&row.retail_price);
        let source = csv_parser::normalize_source(&row.source);
        let (cost, min_price, _vendor) = pricing_engine.calculate_cost(retail_price, &source);

        let item_id = uuid::Uuid::new_v4().to_string();
        let status = if auction_id.is_some() { "Listed" } else { "InStock" };

        db.conn
            .execute(
                "INSERT INTO inventory_items
                 (id, manifest_id, lot_number, raw_title, vendor_code, source,
                  retail_price, cost_price, min_price, quantity, current_status, auction_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                rusqlite::params![
                    item_id,
                    manifest_id,
                    row.lot_number,
                    row.title,
                    row.vendor_code,
                    source,
                    retail_price,
                    cost,
                    min_price,
                    row.quantity.parse::<i32>().unwrap_or(1),
                    status,
                    auction_id.as_ref()
                ],
            )
            .map_err(|e| e.to_string())?;

        // NLP: extract brand, model, category from title
        let entities = nlp_extractor.extract(&row.title);
        db.conn
            .execute(
                "UPDATE inventory_items
                 SET normalized_title = ?1,
                     extracted_brand = ?2,
                     extracted_model = ?3,
                     category = ?4
                 WHERE id = ?5",
                rusqlite::params![
                    entities.normalized_title,
                    entities.brand,
                    entities.model,
                    entities.category,
                    item_id
                ],
            )
            .map_err(|e| e.to_string())?;

        total_retail += retail_price;
        total_cost += cost;
    }

    // 4. Update manifest totals
    db.conn
        .execute(
            "UPDATE manifests SET total_retail_value = ?1, total_cost = ?2 WHERE id = ?3",
            rusqlite::params![total_retail, total_cost, manifest_id],
        )
        .map_err(|e| e.to_string())?;

    Ok(ManifestSummary {
        id: manifest_id,
        items_count: rows.len(),
        total_retail,
        total_cost,
    })
}

#[tauri::command]
fn get_inventory_items(
    status: Option<String>,
    state: tauri::State<AppState>,
) -> Result<Vec<db::InventoryItemRow>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_inventory_items(status.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_dashboard_stats(
    state: tauri::State<AppState>,
) -> Result<db::DashboardStats, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_dashboard_stats().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_vendors(
    state: tauri::State<AppState>,
) -> Result<Vec<pricing::Vendor>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    pricing::PricingEngine::load_vendors(&db.conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_auction_pnl_list(
    state: tauri::State<AppState>,
) -> Result<Vec<AuctionPnlRow>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.conn.prepare(
        "SELECT auction_id, auction_name, start_date,
                total_items, sold_items, buyback_items,
                total_revenue, total_cost, total_commission, net_profit
         FROM v_auction_pnl
         ORDER BY start_date DESC
         LIMIT 12"
    ).map_err(|e| e.to_string())?;

    let rows = stmt.query_map([], |row| {
        Ok(AuctionPnlRow {
            auction_id: row.get(0)?,
            auction_name: row.get(1)?,
            start_date: row.get(2)?,
            total_items: row.get(3)?,
            sold_items: row.get(4)?,
            buyback_items: row.get(5)?,
            total_revenue: row.get(6)?,
            total_cost: row.get(7)?,
            total_commission: row.get(8)?,
            net_profit: row.get(9)?,
        })
    }).map_err(|e| e.to_string())?
    .collect::<rusqlite::Result<Vec<_>>>()
    .map_err(|e| e.to_string())?;

    Ok(rows)
}

#[tauri::command]
fn export_inventory_csv(
    file_path: String,
    status: Option<String>,
    state: tauri::State<AppState>,
) -> Result<usize, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let items = db.get_inventory_items(status.as_deref()).map_err(|e| e.to_string())?;

    let mut wtr = csv::Writer::from_path(&file_path).map_err(|e| e.to_string())?;
    wtr.write_record(["Lot#", "Title", "Source", "Status", "Retail", "Cost", "Min Price", "Created"])
        .map_err(|e| e.to_string())?;

    for item in &items {
        wtr.write_record([
            item.lot_number.as_deref().unwrap_or(""),
            &item.raw_title,
            item.source.as_deref().unwrap_or(""),
            &item.current_status,
            &format!("{:.2}", item.retail_price),
            &format!("{:.2}", item.cost_price),
            &format!("{:.2}", item.min_price),
            &item.created_at,
        ]).map_err(|e| e.to_string())?;
    }
    wtr.flush().map_err(|e| e.to_string())?;
    Ok(items.len())
}

#[tauri::command]
fn update_item_status(
    item_id: String,
    status: String,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn.execute(
        "UPDATE inventory_items SET current_status = ?1, auction_id = CASE WHEN ?1 = 'InStock' THEN NULL ELSE auction_id END WHERE id = ?2",
        rusqlite::params![status, item_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn get_setting(
    key: String,
    state: tauri::State<AppState>,
) -> Result<Option<String>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let result = db.conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        rusqlite::params![key],
        |row| row.get::<_, String>(0),
    );
    match result {
        Ok(val) => Ok(Some(val)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
fn save_setting(
    key: String,
    value: String,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    // SQLite upsert
    db.conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn save_binary_file(
    file_path: String,
    data: Vec<u8>,
) -> Result<(), String> {
    std::fs::write(&file_path, data).map_err(|e| format!("Failed to save file: {}", e))
}

// ============================================================
// Main
// ============================================================

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    log::info!("Starting Sugarland application v0.2.0");

    let db = Database::new("sugarland.db").expect("Failed to initialize database");

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState {
            db: Mutex::new(db),
        })
        .invoke_handler(tauri::generate_handler![
            save_binary_file,
            import_manifest,
            get_inventory_items,
            get_dashboard_stats,
            get_vendors,
            get_auction_pnl_list,
            export_inventory_csv,
            update_item_status,
            get_setting,
            save_setting,
            // Auctions
            auctions::create_auction,
            auctions::get_auctions,
            auctions::export_auction_csv,
            auctions::get_auction_by_id,
            auctions::update_auction_status,
            auctions::update_vendor,
            auctions::unassign_item,
            // Reconciliation
            reconciliation::reconcile_auction,
            reconciliation::get_pl_report,
            // CSV Validation
            csv_parser::validate_csv,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
