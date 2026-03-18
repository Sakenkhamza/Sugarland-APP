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
        let condition = csv_parser::extract_and_normalize_condition(&row.description);

        db.conn
            .execute(
                "INSERT INTO inventory_items
                 (id, manifest_id, lot_number, raw_title, vendor_code, source,
                  retail_price, cost_price, min_price, quantity, current_status, auction_id, condition)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
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
                    auction_id.as_ref(),
                    condition
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
// Condition Types Commands
// ============================================================

#[derive(Debug, Serialize)]
pub struct ConditionType {
    pub id: String,
    pub label: String,
    pub category: String,
}

#[tauri::command]
fn get_condition_types(state: tauri::State<AppState>) -> Result<Vec<ConditionType>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.conn.prepare(
        "SELECT id, label, category FROM condition_types ORDER BY category, label"
    ).map_err(|e| e.to_string())?;

    let types = stmt.query_map([], |row| {
        Ok(ConditionType {
            id: row.get(0)?,
            label: row.get(1)?,
            category: row.get(2)?,
        })
    }).map_err(|e| e.to_string())?
    .collect::<rusqlite::Result<Vec<_>>>()
    .map_err(|e| e.to_string())?;

    Ok(types)
}

// ============================================================
// Source Types Commands
// ============================================================

#[derive(Debug, Serialize)]
pub struct SourceType {
    pub id: String,
    pub name: String,
}

#[tauri::command]
fn get_source_types(state: tauri::State<AppState>) -> Result<Vec<SourceType>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.conn.prepare(
        "SELECT id, name FROM source_types ORDER BY name"
    ).map_err(|e| e.to_string())?;

    let types = stmt.query_map([], |row| {
        Ok(SourceType {
            id: row.get(0)?,
            name: row.get(1)?,
        })
    }).map_err(|e| e.to_string())?
    .collect::<rusqlite::Result<Vec<_>>>()
    .map_err(|e| e.to_string())?;

    Ok(types)
}

#[tauri::command]
fn add_source_type(name: String, state: tauri::State<AppState>) -> Result<String, String> {
    let mut db = state.db.lock().map_err(|e| e.to_string())?;
    let id = name.to_lowercase().replace(' ', "_");
    
    // Begin transaction
    let tx = db.conn.transaction().map_err(|e| e.to_string())?;
    
    tx.execute(
        "INSERT OR IGNORE INTO source_types (id, name) VALUES (?1, ?2)",
        rusqlite::params![id, name],
    ).map_err(|e| e.to_string())?;
    
    tx.execute(
        "INSERT OR IGNORE INTO vendors (id, name, cost_coefficient, min_price_margin, is_active) VALUES (?1, ?2, 0.15, 0.10, 1)",
        rusqlite::params![id, name],
    ).map_err(|e| e.to_string())?;
    
    tx.commit().map_err(|e| e.to_string())?;
    
    Ok(id)
}

#[tauri::command]
fn delete_source_type(name: String, state: tauri::State<AppState>) -> Result<(), String> {
    let mut db = state.db.lock().map_err(|e| e.to_string())?;
    let id = name.to_lowercase().replace(' ', "_");
    
    let tx = db.conn.transaction().map_err(|e| e.to_string())?;
    
    tx.execute(
        "DELETE FROM source_types WHERE name = ?1 OR id = ?2",
        rusqlite::params![name, id],
    ).map_err(|e| e.to_string())?;
    
    tx.execute(
        "UPDATE inventory_items
         SET source = 'Unknown'
         WHERE lower(trim(source)) = lower(trim(?1))
            OR lower(trim(source)) = lower(trim(?2))",
        rusqlite::params![name, id],
    ).map_err(|e| e.to_string())?;

    tx.execute(
        "DELETE FROM vendors WHERE name = ?1 OR id = ?2",
        rusqlite::params![name, id],
    ).map_err(|e| e.to_string())?;
    
    tx.commit().map_err(|e| e.to_string())?;
    
    Ok(())
}

// ============================================================
// Item Field Update Commands
// ============================================================

#[tauri::command]
fn update_item_condition(
    item_id: String,
    condition: String,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn.execute(
        "UPDATE inventory_items SET condition = ?1 WHERE id = ?2",
        rusqlite::params![condition, item_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn update_item_source(
    item_id: String,
    source: String,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn.execute(
        "UPDATE inventory_items SET source = ?1 WHERE id = ?2",
        rusqlite::params![source, item_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn update_item_sale_order(
    item_id: String,
    sale_order: i32,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn.execute(
        "UPDATE inventory_items SET sale_order = ?1 WHERE id = ?2",
        rusqlite::params![sale_order, item_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn update_item_buybacker(
    item_id: String,
    buybacker_id: String,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn.execute(
        "UPDATE inventory_items SET buybacker_id = ?1 WHERE id = ?2",
        rusqlite::params![buybacker_id, item_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

// ============================================================
// Pricing Rules Commands
// ============================================================

#[tauri::command]
fn get_pricing_rules(state: tauri::State<AppState>) -> Result<Vec<pricing::PricingRule>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    pricing::get_pricing_rules(&db.conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_pricing_rule(
    condition_category: String,
    level: i32,
    multiplier: f64,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    pricing::update_pricing_rule(&db.conn, &condition_category, level, multiplier)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn recalculate_prices(
    auction_id: String,
    vendor_costs: std::collections::HashMap<String, f64>,
    condition_margins_by_supplier: Option<std::collections::HashMap<String, std::collections::HashMap<String, f64>>>,
    condition_margins: Option<std::collections::HashMap<String, f64>>,
    state: tauri::State<AppState>,
) -> Result<i32, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Get all items in the auction
    let mut stmt = db.conn.prepare(
        "SELECT id, retail_price, source, condition FROM inventory_items WHERE auction_id = ?1"
    ).map_err(|e| e.to_string())?;

    struct ItemData { id: String, retail: f64, source: String, condition: String }
    let items: Vec<ItemData> = stmt.query_map(
        rusqlite::params![auction_id],
        |row| Ok(ItemData {
            id: row.get(0)?,
            retail: row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
            source: row.get::<_, Option<String>>(2)?.unwrap_or_else(|| "".to_string()),
            condition: row.get::<_, Option<String>>(3)?.unwrap_or_else(|| "New".to_string()),
        })
    ).map_err(|e| e.to_string())?
    .collect::<rusqlite::Result<Vec<_>>>()
    .map_err(|e| e.to_string())?;

    let mut count = 0;
    for item in &items {
        let cost_pct = vendor_costs.get(&item.source).copied().unwrap_or(0.15); // default 15%
        let cost_price = (item.retail * cost_pct * 100.0).round() / 100.0;

        let margin_pct = condition_margins_by_supplier
            .as_ref()
            .and_then(|margins_by_supplier| margins_by_supplier.get(&item.source))
            .and_then(|supplier_margins| supplier_margins.get(&item.condition))
            .copied()
            .or_else(|| {
                condition_margins
                    .as_ref()
                    .and_then(|margins| margins.get(&item.condition))
                    .copied()
            })
            .unwrap_or(0.10); // default 10%
        let new_min_price = (cost_price + (item.retail * margin_pct)).ceil();

        db.conn.execute(
            "UPDATE inventory_items SET min_price = ?1, cost_price = ?2 WHERE id = ?3",
            rusqlite::params![new_min_price, cost_price, item.id],
        ).map_err(|e| e.to_string())?;
        count += 1;
    }

    Ok(count)
}

// ============================================================
// Buy-backer Commands
// ============================================================

#[derive(Debug, Serialize)]
pub struct Buybacker {
    pub id: String,
    pub name: String,
    pub is_active: bool,
}

#[tauri::command]
fn get_buybackers(state: tauri::State<AppState>) -> Result<Vec<Buybacker>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.conn.prepare(
        "SELECT id, name, is_active FROM buybackers ORDER BY name"
    ).map_err(|e| e.to_string())?;

    let items = stmt.query_map([], |row| {
        Ok(Buybacker {
            id: row.get(0)?,
            name: row.get(1)?,
            is_active: row.get(2)?,
        })
    }).map_err(|e| e.to_string())?
    .collect::<rusqlite::Result<Vec<_>>>()
    .map_err(|e| e.to_string())?;

    Ok(items)
}

#[tauri::command]
fn add_buybacker(
    name: String,
    state: tauri::State<AppState>,
) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    db.conn.execute(
        "INSERT INTO buybackers (id, name) VALUES (?1, ?2)",
        rusqlite::params![id, name],
    ).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
fn update_buybacker(
    id: String,
    name: String,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn.execute(
        "UPDATE buybackers SET name = ?1 WHERE id = ?2",
        rusqlite::params![name, id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn delete_buybacker(
    id: String,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.conn.execute(
        "DELETE FROM buybackers WHERE id = ?1",
        rusqlite::params![id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

// ============================================================
// Item History (Repeaters) Command
// ============================================================

#[derive(Debug, Serialize)]
pub struct ItemHistoryEntry {
    pub auction_name: String,
    pub lot_number: Option<String>,
    pub high_bid: f64,
    pub sale_date: String,
    pub bidder_name: String,
    pub is_buyback: bool,
}

#[tauri::command]
fn get_item_history(
    normalized_title: String,
    state: tauri::State<AppState>,
) -> Result<Vec<ItemHistoryEntry>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.conn.prepare(
        "SELECT a.name, i.lot_number, ar.high_bid, 
                COALESCE(i.sold_at, ar.created_at) as sale_date,
                ar.winning_bidder, ar.is_buyback
         FROM auction_results ar
         JOIN inventory_items i ON ar.item_id = i.id
         JOIN auctions a ON ar.auction_id = a.id
         WHERE i.normalized_title = ?1
         ORDER BY ar.created_at DESC
         LIMIT 20"
    ).map_err(|e| e.to_string())?;

    let entries = stmt.query_map(
        rusqlite::params![normalized_title],
        |row| Ok(ItemHistoryEntry {
            auction_name: row.get(0)?,
            lot_number: row.get(1)?,
            high_bid: row.get(2)?,
            sale_date: row.get(3)?,
            bidder_name: row.get(4)?,
            is_buyback: row.get(5)?,
        })
    ).map_err(|e| e.to_string())?
    .collect::<rusqlite::Result<Vec<_>>>()
    .map_err(|e| e.to_string())?;

    Ok(entries)
}

// ============================================================
// Maintenance Commands
// ============================================================

#[tauri::command]
fn wipe_database(state: tauri::State<AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    
    db.conn.execute_batch(
        "
        DELETE FROM auction_reports;
        DELETE FROM auction_results;
        DELETE FROM historical_sales;
        DELETE FROM inventory_items;
        DELETE FROM auctions;
        DELETE FROM manifests;
        PRAGMA optimize;
        "
    ).map_err(|e| e.to_string())?;
    
    Ok(())
}

// ============================================================
// Main
// ============================================================

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    log::info!("Starting Sugarland application v0.3.0");

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
            // Condition / Source types
            get_condition_types,
            get_source_types,
            add_source_type,
            delete_source_type,
            // Item field updates
            update_item_condition,
            update_item_source,
            update_item_sale_order,
            update_item_buybacker,
            // Pricing rules
            get_pricing_rules,
            update_pricing_rule,
            recalculate_prices,
            // Buy-backers
            get_buybackers,
            add_buybacker,
            update_buybacker,
            delete_buybacker,
            // Item history (Repeaters)
            get_item_history,
            // Auctions
            auctions::create_auction,
            auctions::get_auctions,
            auctions::get_auction_by_id,
            auctions::update_auction_status,
            auctions::update_vendor,
            auctions::unassign_item,
            auctions::get_relistable_inventory_items,
            auctions::assign_items_to_auction,
            auctions::finish_auction,
            auctions::get_auction_reports,
            auctions::get_all_auction_reports,
            auctions::get_item_repeater_stats,
            auctions::get_item_first_auction_map,
            auctions::open_report_file,
            auctions::rename_auction,
            auctions::delete_auction,
            // Reconciliation
            reconciliation::reconcile_auction,
            reconciliation::get_pl_report,
            // CSV Validation
            csv_parser::validate_csv,
            wipe_database,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
