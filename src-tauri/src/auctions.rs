use crate::db::Database;
use rusqlite::Result;
use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Formula, Workbook};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};
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
    #[serde(default)]
    pub vendor_id: Option<String>,
    #[serde(default)]
    pub hibid_auction_id: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateVendorRequest {
    pub cost_coefficient: f64,
    pub min_price_margin: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuctionReport {
    pub id: String,
    pub auction_id: String,
    pub auction_name: String,
    pub report_type: String,
    pub file_name: String,
    pub file_path: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct FinishAuctionResult {
    pub detail_report: String,
    pub summary_report: String,
}

#[derive(Debug, Serialize)]
pub struct AuctionResultBid {
    pub item_id: String,
    pub high_bid: f64,
}

const REPORT_BONUS_RATE: f64 = 0.11;
const REPORT_HISTORY_WINDOW: u32 = 8;

// Internal struct for report data
#[derive(Debug, Clone)]
struct ReportItem {
    item_id: String,
    lot_number: String,
    sale_order: Option<i32>,
    title: String,
    retail_price: f64,
    source: String,
    condition: String,
    cost_coefficient: f64,
    min_price_pct: f64,
    cost_price: f64,
    high_bid: f64,
    max_bid: f64,
    selling_price: f64,
    bidder_id: String,
    buyer: String,
    bidder_email: String,
    bidder_phone: String,
    is_buyback: bool,
    status: String,
}

#[derive(Debug, Clone)]
struct HibidStatRow {
    lot_number: String,
    title: String,
    season_header: String,
    bidder_id: String,
    buyer_name: String,
    bidder_email: String,
    bidder_phone: String,
    high_bid_cents: f64,
    max_bid_cents: f64,
}

pub struct AuctionManager;

fn extract_auction_number(raw: &str) -> Option<u32> {
    let mut groups: Vec<String> = Vec::new();
    let mut current = String::new();

    for ch in raw.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else if !current.is_empty() {
            groups.push(current.clone());
            current.clear();
        }
    }

    if !current.is_empty() {
        groups.push(current);
    }

    groups
        .last()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|n| *n > 0)
}

fn normalize_auction_name(raw: &str) -> Option<String> {
    extract_auction_number(raw).map(|n| format!("Sugarland {}", n))
}

fn build_report_sheet_name(auction_name: &str) -> String {
    match extract_auction_number(auction_name) {
        Some(number) => format!("S-{}", number),
        None => "S-0".to_string(),
    }
}

fn build_history_headers(auction_name: &str) -> Vec<String> {
    match extract_auction_number(auction_name) {
        Some(auction_number) if auction_number > 1 => {
            let end = auction_number - 1;
            let start = if end >= REPORT_HISTORY_WINDOW {
                end - (REPORT_HISTORY_WINDOW - 1)
            } else {
                1
            };
            (start..=end).map(|n| format!("S{}", n)).collect()
        }
        _ => (1..=REPORT_HISTORY_WINDOW)
            .map(|n| format!("S{}", n))
            .collect(),
    }
}

fn build_lookup_name_header(auction_name: &str) -> String {
    extract_auction_number(auction_name)
        .and_then(|n| n.checked_sub(2))
        .map(|n| format!("{} -name", n))
        .unwrap_or_else(|| "buyer name".to_string())
}

fn excel_col_name(index: u16) -> String {
    let mut col = index as u32;
    let mut chars: Vec<char> = Vec::new();
    loop {
        let rem = (col % 26) as u8;
        chars.push((b'A' + rem) as char);
        if col < 26 {
            break;
        }
        col = (col / 26) - 1;
    }
    chars.iter().rev().collect()
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn formula_result_string(value: f64) -> String {
    let mut text = round2(value).to_string();
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        "0".to_string()
    } else {
        text
    }
}

fn normalize_path_key(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

fn split_natural_tokens(value: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_is_digit: Option<bool> = None;

    for ch in value.chars() {
        let is_digit = ch.is_ascii_digit();
        match current_is_digit {
            Some(prev) if prev == is_digit => current.push(ch),
            Some(_) => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                }
                current.clear();
                current.push(ch);
                current_is_digit = Some(is_digit);
            }
            None => {
                current.push(ch);
                current_is_digit = Some(is_digit);
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn natural_lot_cmp(a: &str, b: &str) -> Ordering {
    let a_parts = split_natural_tokens(a);
    let b_parts = split_natural_tokens(b);
    let max_len = a_parts.len().max(b_parts.len());

    for idx in 0..max_len {
        let a_part = a_parts.get(idx).map(String::as_str).unwrap_or("");
        let b_part = b_parts.get(idx).map(String::as_str).unwrap_or("");

        match (a_part.parse::<i64>(), b_part.parse::<i64>()) {
            (Ok(a_num), Ok(b_num)) => {
                let cmp = a_num.cmp(&b_num);
                if cmp != Ordering::Equal {
                    return cmp;
                }
            }
            _ => {
                let cmp = a_part.to_lowercase().cmp(&b_part.to_lowercase());
                if cmp != Ordering::Equal {
                    return cmp;
                }
            }
        }
    }

    Ordering::Equal
}

fn build_sale_order_index(items: &[ReportItem]) -> HashMap<String, i32> {
    let mut natural = items.to_vec();
    natural.sort_by(|a, b| {
        natural_lot_cmp(&a.lot_number, &b.lot_number).then_with(|| a.item_id.cmp(&b.item_id))
    });

    let mut result = HashMap::new();
    for (index, item) in natural.iter().enumerate() {
        result.insert(item.item_id.clone(), index as i32 + 1);
    }
    result
}

impl AuctionManager {
    pub fn create_auction(db: &Database, req: CreateAuctionRequest) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let normalized_name = normalize_auction_name(&req.name).ok_or_else(|| {
            rusqlite::Error::InvalidParameterName(
                "Invalid auction number. Expected format: Sugarland <number>".to_string(),
            )
        })?;

        db.conn.execute(
            "INSERT INTO auctions (id, hibid_auction_id, name, vendor_id, start_date, end_date, status, total_lots)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'Active', 0)",
            rusqlite::params![
                id,
                req.hibid_auction_id,
                normalized_name,
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

        let auctions = stmt
            .query_map([], |row| {
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

    fn load_hibid_stat_rows(db: &Database) -> std::result::Result<Vec<HibidStatRow>, String> {
        let mut stmt = db
            .conn
            .prepare(
                "SELECT
                    COALESCE(i.lot_number, ''),
                    i.raw_title,
                    a.name,
                    COALESCE(ar.bidder_id, ''),
                    COALESCE(ar.winning_bidder, ''),
                    COALESCE(ar.bidder_email, ''),
                    COALESCE(ar.bidder_phone, ''),
                    COALESCE(ar.high_bid, 0),
                    COALESCE(ar.max_bid, 0)
                FROM auction_results ar
                JOIN inventory_items i ON i.id = ar.item_id
                JOIN auctions a ON a.id = ar.auction_id
                WHERE COALESCE(ar.high_bid, 0) >= 0
                ORDER BY datetime(ar.created_at) ASC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, f64>(7)?,
                    row.get::<_, f64>(8)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        let mut result = Vec::new();
        for row in rows {
            let (
                lot_number,
                title,
                auction_name,
                bidder_id,
                buyer_name,
                bidder_email,
                bidder_phone,
                high_bid,
                max_bid,
            ) = row.map_err(|e| e.to_string())?;

            let Some(auction_number) = extract_auction_number(&auction_name) else {
                continue;
            };
            let high_bid_cents = (high_bid * 100.0).round();
            let max_bid_cents = if max_bid > 0.0 {
                (max_bid * 100.0).round()
            } else {
                high_bid_cents
            };

            result.push(HibidStatRow {
                lot_number,
                title,
                season_header: format!("S{}", auction_number),
                bidder_id,
                buyer_name,
                bidder_email,
                bidder_phone,
                high_bid_cents,
                max_bid_cents,
            });
        }

        Ok(result)
    }

    pub fn finish_auction(
        db: &Database,
        auction_id: &str,
        results_csv_path: &str,
        app_data_dir: &str,
    ) -> std::result::Result<FinishAuctionResult, String> {
        use crate::csv_parser;

        // 1. Get auction info
        let auction = Self::get_auction_by_id(db, auction_id).map_err(|e| e.to_string())?;
        if auction.status != "Active" && auction.status != "Completed" {
            return Err("Auction is not in Active or Completed status".to_string());
        }

        // 2. Parse the HiBid results CSV
        let csv_results = csv_parser::parse_hibid_results(results_csv_path)
            .map_err(|e| format!("Failed to parse HiBid CSV: {}", e))?;

        log::info!("Parsed {} rows from HiBid results CSV", csv_results.len());

        // Build a map of lot_number -> CSV row for matching
        let mut csv_by_lot: HashMap<String, &csv_parser::HiBidResultRow> = HashMap::new();
        for row in &csv_results {
            let lot = row.lot_number.trim().to_string();
            if !lot.is_empty() {
                csv_by_lot.insert(lot, row);
            }
        }

        // 3. Get all inventory items for this auction
        let mut item_stmt = db
            .conn
            .prepare(
                "SELECT id, lot_number, raw_title, retail_price, source,
                    cost_price, min_price, sale_order, condition
             FROM inventory_items
             WHERE auction_id = ?1
             ORDER BY lot_number",
            )
            .map_err(|e| e.to_string())?;

        struct ItemInfo {
            id: String,
            lot_number: String,
            title: String,
            retail_price: f64,
            source: String,
            cost_price: f64,
            min_price: f64,
            sale_order: Option<i32>,
            condition: String,
        }

        let db_items: Vec<ItemInfo> = item_stmt
            .query_map(rusqlite::params![auction_id], |row| {
                Ok(ItemInfo {
                    id: row.get(0)?,
                    lot_number: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    title: row.get(2)?,
                    retail_price: row.get(3)?,
                    source: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                    cost_price: row.get(5)?,
                    min_price: row.get(6)?,
                    sale_order: row.get(7)?,
                    condition: row.get::<_, Option<String>>(8)?.unwrap_or_default(),
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;

        // 4. Drop the statement (borrow checker) before mutating db
        drop(item_stmt);

        // 4.5 Load active buybackers from database
        let mut bb_stmt = db
            .conn
            .prepare("SELECT name FROM buybackers WHERE is_active = 1")
            .map_err(|e| e.to_string())?;
        let buyback_names: Vec<String> = bb_stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .map(|n| n.trim().to_lowercase())
            .collect();

        // 5. Reconcile: match CSV results to items, update statuses, insert auction_results
        let mut report_items: Vec<ReportItem> = Vec::new();
        let mut matched_csv_lots: HashSet<String> = HashSet::new();
        let parse_csv_row = |row: &csv_parser::HiBidResultRow| {
            let bid = csv_parser::clean_hibid_cents_price(&row.high_bid);
            let parsed_max_bid = row
                .max_bid
                .as_deref()
                .map(csv_parser::clean_hibid_cents_price)
                .unwrap_or(bid);
            let bidder_id = row.bidder_id.trim().to_string();
            let buyer_name = row.winning_bidder.trim().to_string();
            let bidder_email = row
                .email
                .as_ref()
                .map(|v| v.trim().to_string())
                .unwrap_or_default();
            let bidder_phone = row
                .phone
                .as_ref()
                .map(|v| v.trim().to_string())
                .unwrap_or_default();

            let is_floor = buyer_name.eq_ignore_ascii_case("floor");
            let has_identity = !buyer_name.is_empty() || !bidder_id.is_empty();
            let has_valid_bidder = !is_floor && bid > 0.0 && has_identity;
            let buyer_display = if !buyer_name.is_empty() {
                buyer_name.clone()
            } else if bid <= 0.0 {
                "Floor".to_string()
            } else {
                bidder_id.clone()
            };

            if has_valid_bidder {
                (
                    bid,
                    if parsed_max_bid > 0.0 {
                        parsed_max_bid
                    } else {
                        bid
                    },
                    bidder_id,
                    buyer_display,
                    bidder_email,
                    bidder_phone,
                    true,
                )
            } else {
                (
                    0.0,
                    0.0,
                    bidder_id,
                    buyer_display,
                    bidder_email,
                    bidder_phone,
                    false,
                )
            }
        };

        // First, clean up any existing auction_results for this auction
        db.conn
            .execute(
                "DELETE FROM auction_results WHERE auction_id = ?1",
                rusqlite::params![auction_id],
            )
            .map_err(|e| e.to_string())?;

        for item in &db_items {
            let lot_key = item.lot_number.trim().to_string();
            let csv_row = csv_by_lot.get(lot_key.as_str());
            if csv_row.is_some() {
                matched_csv_lots.insert(lot_key);
            }

            let (high_bid, max_bid, bidder_id, buyer, bidder_email, bidder_phone, has_valid_bidder) =
                match csv_row {
                    Some(row) => parse_csv_row(row),
                    None => (
                        0.0,
                        0.0,
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                        false,
                    ),
                };

            let is_buyback = if has_valid_bidder {
                let winner_lower = buyer.to_lowercase();
                buyback_names
                    .iter()
                    .any(|bb_name| winner_lower.contains(bb_name))
            } else {
                false
            };

            // Determine item status
            let new_status = if is_buyback {
                "Buyback"
            } else if has_valid_bidder {
                "Sold"
            } else {
                "Unsold"
            };
            let selling_price = if new_status == "Sold" { high_bid } else { 0.0 };

            // Update item status
            db.conn
                .execute(
                    "UPDATE inventory_items
                 SET current_status = ?1,
                     sold_at = CASE WHEN ?1 = 'Sold' THEN CURRENT_TIMESTAMP ELSE NULL END
                 WHERE id = ?2",
                    rusqlite::params![new_status, item.id],
                )
                .map_err(|e| e.to_string())?;

            // Insert auction result
            let result_id = Uuid::new_v4().to_string();
            let commission = if new_status == "Sold" {
                selling_price * 0.15
            } else {
                0.0
            };
            let net_profit = if new_status == "Sold" {
                selling_price - item.cost_price
            } else {
                0.0
            };
            db.conn
                .execute(
                    "INSERT INTO auction_results (
                    id, auction_id, item_id, winning_bidder, bidder_id, high_bid, max_bid,
                    bidder_email, bidder_phone, is_buyback, commission_rate, commission_amount,
                    net_profit, item_status, min_price_snapshot
                )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                    rusqlite::params![
                        result_id,
                        auction_id,
                        item.id,
                        buyer,
                        bidder_id,
                        high_bid,
                        if max_bid > 0.0 { max_bid } else { high_bid },
                        bidder_email,
                        bidder_phone,
                        is_buyback,
                        0.15_f64,
                        commission,
                        net_profit,
                        new_status,
                        item.min_price,
                    ],
                )
                .map_err(|e| e.to_string())?;

            let cost_coefficient = if item.retail_price > 0.0 {
                round2(item.cost_price / item.retail_price).max(0.0)
            } else {
                0.0
            };
            let min_price_pct = if item.retail_price > 0.0 {
                round2(((item.min_price - item.cost_price) / item.retail_price).max(0.0))
            } else {
                0.10
            };
            let retail_price = item.retail_price.round();
            let cost_price = round2(retail_price * cost_coefficient);

            report_items.push(ReportItem {
                item_id: item.id.clone(),
                lot_number: item.lot_number.clone(),
                sale_order: item.sale_order,
                title: item.title.clone(),
                retail_price,
                source: item.source.clone(),
                condition: item.condition.clone(),
                cost_coefficient,
                min_price_pct,
                cost_price,
                high_bid,
                max_bid: if max_bid > 0.0 { max_bid } else { high_bid },
                selling_price,
                bidder_id,
                buyer,
                bidder_email,
                bidder_phone,
                is_buyback,
                status: new_status.to_string(),
            });
        }

        // Add rows that exist in HiBid CSV but don't exist in inventory.
        // These rows are included only in the detail report to match HiBid row count.
        let mut detail_report_items = report_items.clone();
        let mut unmatched_csv_rows = 0;
        let mut unmatched_difference_total = 0.0;
        for row in &csv_results {
            let lot_number = row.lot_number.trim().to_string();
            if lot_number.is_empty() || matched_csv_lots.contains(&lot_number) {
                continue;
            }

            let (high_bid, max_bid, bidder_id, buyer, bidder_email, bidder_phone, has_valid_bidder) =
                parse_csv_row(row);
            let is_buyback = if has_valid_bidder {
                let winner_lower = buyer.to_lowercase();
                buyback_names
                    .iter()
                    .any(|bb_name| winner_lower.contains(bb_name))
            } else {
                false
            };
            let status = if is_buyback {
                "Buyback"
            } else if has_valid_bidder {
                "Sold"
            } else {
                "Unsold"
            };
            let selling_price = if status == "Sold" { high_bid } else { 0.0 };
            let title = row
                .title
                .as_ref()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| format!("Lot {}", lot_number));

            detail_report_items.push(ReportItem {
                item_id: Uuid::new_v4().to_string(),
                lot_number,
                sale_order: None,
                title,
                retail_price: 0.0,
                source: "Unknown".to_string(),
                condition: String::new(),
                cost_coefficient: 0.0,
                min_price_pct: 0.0,
                cost_price: 0.0,
                high_bid,
                max_bid: if max_bid > 0.0 { max_bid } else { high_bid },
                selling_price,
                bidder_id,
                buyer,
                bidder_email,
                bidder_phone,
                is_buyback,
                status: status.to_string(),
            });
            if status == "Sold" {
                unmatched_difference_total += high_bid;
            }
            unmatched_csv_rows += 1;
        }
        if unmatched_csv_rows > 0 {
            log::warn!(
                "Added {} unmatched HiBid rows to detail report for auction {}",
                unmatched_csv_rows,
                auction_id
            );
        }
        let unmatched_adjustment_key = format!("auction_unmatched_diff_{}", auction_id);
        db.conn
            .execute(
                "INSERT INTO settings (key, value, description, category)
                 VALUES (?1, ?2, ?3, 'system')
                 ON CONFLICT(key) DO UPDATE SET
                    value = excluded.value,
                    description = excluded.description,
                    category = excluded.category",
                rusqlite::params![
                    unmatched_adjustment_key,
                    format!("{:.4}", unmatched_difference_total),
                    "Auto-generated unmatched HiBid difference adjustment",
                ],
            )
            .map_err(|e| e.to_string())?;

        // 6. Create reports directory
        let reports_dir = format!("{}/reports/{}", app_data_dir, auction_id);
        std::fs::create_dir_all(&reports_dir)
            .map_err(|e| format!("Failed to create reports dir: {}", e))?;

        let safe_name = auction.name.replace(
            |c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c != ' ',
            "",
        );

        // 7. Generate Report 1 (detailed per-item report)
        let detail_file_name = format!("Отчет_{}.xlsx", safe_name);
        let detail_file_path = format!("{}/{}", reports_dir, detail_file_name);
        let hibid_stat_rows = Self::load_hibid_stat_rows(db)?;
        Self::generate_detail_report(
            &auction.name,
            &detail_report_items,
            &hibid_stat_rows,
            &detail_file_path,
        )?;

        // 8. Generate Report 2 (summary report)
        let summary_file_name = format!("Сводный_отчет_{}.xlsx", safe_name);
        let summary_file_path = format!("{}/{}", reports_dir, summary_file_name);
        Self::generate_summary_report(&auction.name, &report_items, &summary_file_path)?;

        // 9. Update auction status to Completed
        db.conn
            .execute(
                "UPDATE auctions SET status = 'Completed' WHERE id = ?1",
                rusqlite::params![auction_id],
            )
            .map_err(|e| e.to_string())?;

        // 10. Remove previous report records/files for this auction and save fresh ones.
        let mut old_reports_stmt = db
            .conn
            .prepare("SELECT file_path FROM auction_reports WHERE auction_id = ?1")
            .map_err(|e| e.to_string())?;
        let old_report_paths: Vec<String> = old_reports_stmt
            .query_map(rusqlite::params![auction_id], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;
        drop(old_reports_stmt);

        let detail_key = normalize_path_key(&detail_file_path);
        let summary_key = normalize_path_key(&summary_file_path);
        for old_path in &old_report_paths {
            let old_key = normalize_path_key(old_path);
            if old_key == detail_key || old_key == summary_key {
                continue;
            }
            if let Err(err) = std::fs::remove_file(old_path) {
                if err.kind() != std::io::ErrorKind::NotFound {
                    log::warn!(
                        "Failed to remove old report file for auction {} ({}): {}",
                        auction_id,
                        old_path,
                        err
                    );
                }
            }
        }

        db.conn
            .execute(
                "DELETE FROM auction_reports WHERE auction_id = ?1",
                rusqlite::params![auction_id],
            )
            .map_err(|e| e.to_string())?;

        let detail_id = Uuid::new_v4().to_string();
        let summary_id = Uuid::new_v4().to_string();
        db.conn.execute(
            "INSERT INTO auction_reports (id, auction_id, report_type, file_name, file_path) VALUES (?1, ?2, 'detail', ?3, ?4)",
            rusqlite::params![detail_id, auction_id, detail_file_name, detail_file_path],
        ).map_err(|e| e.to_string())?;
        db.conn.execute(
            "INSERT INTO auction_reports (id, auction_id, report_type, file_name, file_path) VALUES (?1, ?2, 'summary', ?3, ?4)",
            rusqlite::params![summary_id, auction_id, summary_file_name, summary_file_path],
        ).map_err(|e| e.to_string())?;

        log::info!(
            "Auction {} finished. Reports: {}, {}",
            auction_id,
            detail_file_name,
            summary_file_name
        );

        Ok(FinishAuctionResult {
            detail_report: detail_file_name,
            summary_report: summary_file_name,
        })
    }

    fn generate_detail_report(
        auction_name: &str,
        items: &[ReportItem],
        hibid_stat_rows: &[HibidStatRow],
        file_path: &str,
    ) -> std::result::Result<(), String> {
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();

        let detail_sheet_name = build_report_sheet_name(auction_name);
        worksheet
            .set_name(detail_sheet_name.get(..31).unwrap_or(&detail_sheet_name))
            .map_err(|e| e.to_string())?;

        let header_format = Format::new()
            .set_bold()
            .set_border(FormatBorder::Thin)
            .set_background_color(Color::RGB(0xD9E1F2))
            .set_align(FormatAlign::Center);
        let data_format = Format::new().set_border(FormatBorder::Thin);
        let buyback_data_format = Format::new()
            .set_border(FormatBorder::Thin)
            .set_background_color(Color::RGB(0xFFF200));
        let number_format = Format::new()
            .set_border(FormatBorder::Thin)
            .set_num_format("#,##0");
        let percent_format = Format::new()
            .set_border(FormatBorder::Thin)
            .set_num_format("0%");

        let history_headers = build_history_headers(auction_name);
        let derived_sale_order = build_sale_order_index(items);
        let mut sorted_items = items.to_vec();
        sorted_items.sort_by(|a, b| {
            a.source
                .to_lowercase()
                .cmp(&b.source.to_lowercase())
                .then(
                    a.sale_order
                        .unwrap_or(*derived_sale_order.get(&a.item_id).unwrap_or(&i32::MAX))
                        .cmp(
                            &b.sale_order.unwrap_or(
                                *derived_sale_order.get(&b.item_id).unwrap_or(&i32::MAX),
                            ),
                        ),
                )
                .then_with(|| natural_lot_cmp(&a.lot_number, &b.lot_number))
        });

        let name_header = build_lookup_name_header(auction_name);
        let mut headers: Vec<String> = vec![
            "LotNumber".to_string(),
            "sale order".to_string(),
            "Title".to_string(),
            "Retail Price".to_string(),
            "Source".to_string(),
            "Condition".to_string(),
            "cost".to_string(),
            "min price %".to_string(),
            "cost price".to_string(),
            "min pr (+0,5)".to_string(),
            "min pr (+1)".to_string(),
            "".to_string(),
            name_header,
            "sale".to_string(),
            "sale-max".to_string(),
            "difference".to_string(),
            "plus bonus".to_string(),
            "Пред.Макс".to_string(),
        ];
        headers.extend(history_headers.iter().cloned());

        for (col, header) in headers.iter().enumerate() {
            worksheet
                .write_string_with_format(0, col as u16, header, &header_format)
                .map_err(|e| e.to_string())?;
        }

        let reference_widths = [
            8.8867, 8.8867, 67.1094, 8.8867, 14.1094, 5.4414, 8.8867, 10.7773, 11.1094, 14.1094,
            12.4414, 12.4414, 21.5547, 9.1094, 12.4414, 13.0, 13.4414, 11.0, 8.8867, 13.0, 13.0,
            13.0, 13.0, 13.0, 13.0, 13.0,
        ];
        for col in 0..headers.len() {
            let width = reference_widths.get(col).copied().unwrap_or(13.0);
            worksheet
                .set_column_width(col as u16, width)
                .map_err(|e| e.to_string())?;
        }

        let history_start_col: u16 = 18;
        let history_end_col: u16 = if history_headers.is_empty() {
            history_start_col
        } else {
            history_start_col + history_headers.len() as u16 - 1
        };
        let history_end_col_name = excel_col_name(history_end_col);
        let mut history_max_by_title: HashMap<String, HashMap<String, f64>> = HashMap::new();
        for entry in hibid_stat_rows {
            let season_map = history_max_by_title.entry(entry.title.clone()).or_default();
            let value = round2(entry.max_bid_cents / 100.0);
            match season_map.get_mut(&entry.season_header) {
                Some(existing) => {
                    if value > *existing {
                        *existing = value;
                    }
                }
                None => {
                    season_map.insert(entry.season_header.clone(), value);
                }
            }
        }

        for (index, item) in sorted_items.iter().enumerate() {
            let row = (index + 1) as u32;
            let excel_row = row + 1;
            let effective_sale_order = item.sale_order.unwrap_or(
                *derived_sale_order
                    .get(&item.item_id)
                    .unwrap_or(&(index as i32 + 1)),
            );
            let cost_price = round2(item.retail_price * item.cost_coefficient);
            let min_price_half = round2(cost_price + item.retail_price * item.min_price_pct / 2.0);
            let min_price_one = round2(cost_price + item.retail_price * item.min_price_pct);
            let sale_value = round2(item.high_bid);
            let sale_max_value = round2(if item.max_bid > 0.0 {
                item.max_bid
            } else {
                item.high_bid
            });
            let difference_value = round2(sale_value - cost_price);
            let plus_bonus_value = round2(sale_value * REPORT_BONUS_RATE + difference_value);
            let history_values: Vec<f64> = history_headers
                .iter()
                .map(|header| {
                    history_max_by_title
                        .get(&item.title)
                        .and_then(|by_season| by_season.get(header))
                        .copied()
                        .unwrap_or(0.0)
                })
                .collect();
            let repeat_value = history_values.iter().copied().fold(0.0_f64, f64::max);

            worksheet
                .write_string_with_format(row, 0, &item.lot_number, &data_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(row, 1, effective_sale_order as f64, &number_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_string_with_format(row, 2, &item.title, &data_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(row, 3, item.retail_price, &number_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_string_with_format(row, 4, &item.source, &data_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_string_with_format(row, 5, &item.condition, &data_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(row, 6, item.cost_coefficient, &percent_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(row, 7, item.min_price_pct, &percent_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_formula_with_format(
                    row,
                    8,
                    Formula::new(format!("=ROUND(D{}*G{}, 2)", excel_row, excel_row))
                        .set_result(formula_result_string(cost_price)),
                    &number_format,
                )
                .map_err(|e| e.to_string())?;
            worksheet
                .write_formula_with_format(
                    row,
                    9,
                    Formula::new(format!("=I{}+$D{}*H{}/2", excel_row, excel_row, excel_row))
                        .set_result(formula_result_string(min_price_half)),
                    &number_format,
                )
                .map_err(|e| e.to_string())?;
            worksheet
                .write_formula_with_format(
                    row,
                    10,
                    Formula::new(format!("=$I{}+$D{}*$H{}", excel_row, excel_row, excel_row))
                        .set_result(formula_result_string(min_price_one)),
                    &number_format,
                )
                .map_err(|e| e.to_string())?;
            worksheet
                .write_string_with_format(row, 11, "", &data_format)
                .map_err(|e| e.to_string())?;
            let buyer_cell_format = if item.is_buyback {
                &buyback_data_format
            } else {
                &data_format
            };
            worksheet
                .write_string_with_format(row, 12, &item.buyer, buyer_cell_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(row, 13, sale_value, &number_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(row, 14, sale_max_value, &number_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_formula_with_format(
                    row,
                    15,
                    Formula::new(format!("=N{}-I{}", excel_row, excel_row))
                        .set_result(formula_result_string(difference_value)),
                    &number_format,
                )
                .map_err(|e| e.to_string())?;
            worksheet
                .write_formula_with_format(
                    row,
                    16,
                    Formula::new(format!(
                        "=N{}*{:.2}+P{}",
                        excel_row, REPORT_BONUS_RATE, excel_row
                    ))
                    .set_result(formula_result_string(plus_bonus_value)),
                    &number_format,
                )
                .map_err(|e| e.to_string())?;
            if !history_headers.is_empty() {
                worksheet
                    .write_formula_with_format(
                        row,
                        17,
                        Formula::new(format!(
                            "=MAX(S{}:{}{})",
                            excel_row, history_end_col_name, excel_row
                        ))
                        .set_result(formula_result_string(repeat_value)),
                        &number_format,
                    )
                    .map_err(|e| e.to_string())?;
            } else {
                worksheet
                    .write_number_with_format(row, 17, 0.0, &number_format)
                    .map_err(|e| e.to_string())?;
            }

            for (hist_idx, _) in history_headers.iter().enumerate() {
                let col = history_start_col + hist_idx as u16;
                let col_name = excel_col_name(col);
                let history_value = history_values.get(hist_idx).copied().unwrap_or(0.0);
                worksheet
                    .write_formula_with_format(
                        row,
                        col,
                        Formula::new(format!(
                            "=IFERROR(MAXIFS('Hibid stat'!$M:$M,'Hibid stat'!$B:$B,$C{},'Hibid stat'!$C:$C,{}$1)/100,0)",
                            excel_row, col_name
                        ))
                        .set_result(formula_result_string(history_value)),
                        &number_format,
                    )
                    .map_err(|e| e.to_string())?;
            }
        }

        let data_last_row = sorted_items.len() as u32;
        let data_last_col = if headers.is_empty() {
            0
        } else {
            headers.len() as u16 - 1
        };
        worksheet
            .autofilter(0, 0, data_last_row, data_last_col)
            .map_err(|e| e.to_string())?;

        let lookup_sheet = workbook.add_worksheet();
        lookup_sheet
            .set_name("Auction results")
            .map_err(|e| e.to_string())?;
        let lookup_headers = [
            "Lot",
            "Title",
            "Time Left",
            "Reserve",
            "Winning Bidder",
            "Name",
            "Address",
            "State",
            "Zip Code",
            "Email",
            "Phone",
            "Phone 2",
            "High Bid",
            "Max Bid",
            "Bids",
            "Views",
            "Watches",
        ];
        for (col, header) in lookup_headers.iter().enumerate() {
            lookup_sheet
                .write_string(0, col as u16, *header)
                .map_err(|e| e.to_string())?;
        }
        let current_season = extract_auction_number(auction_name)
            .map(|n| format!("S{}", n))
            .unwrap_or_default();
        for (index, item) in sorted_items.iter().enumerate() {
            let row = (index + 1) as u32;
            lookup_sheet
                .write_string(row, 0, &item.lot_number)
                .map_err(|e| e.to_string())?;
            lookup_sheet
                .write_string(row, 1, &item.title)
                .map_err(|e| e.to_string())?;
            lookup_sheet
                .write_string(row, 2, &current_season)
                .map_err(|e| e.to_string())?;
            lookup_sheet
                .write_string(row, 4, &item.bidder_id)
                .map_err(|e| e.to_string())?;
            lookup_sheet
                .write_string(row, 5, &item.buyer)
                .map_err(|e| e.to_string())?;
            lookup_sheet
                .write_string(row, 9, &item.bidder_email)
                .map_err(|e| e.to_string())?;
            lookup_sheet
                .write_string(row, 10, &item.bidder_phone)
                .map_err(|e| e.to_string())?;
            lookup_sheet
                .write_number(row, 12, (item.high_bid * 100.0).round())
                .map_err(|e| e.to_string())?;
            lookup_sheet
                .write_number(
                    row,
                    13,
                    (if item.max_bid > 0.0 {
                        item.max_bid
                    } else {
                        item.high_bid
                    } * 100.0)
                        .round(),
                )
                .map_err(|e| e.to_string())?;
        }
        lookup_sheet
            .autofilter(0, 0, sorted_items.len() as u32, 16)
            .map_err(|e| e.to_string())?;

        let hibid_sheet = workbook.add_worksheet();
        hibid_sheet
            .set_name("Hibid stat")
            .map_err(|e| e.to_string())?;
        for (col, header) in lookup_headers.iter().enumerate() {
            hibid_sheet
                .write_string(0, col as u16, *header)
                .map_err(|e| e.to_string())?;
        }
        for (index, entry) in hibid_stat_rows.iter().enumerate() {
            let row = (index + 1) as u32;
            hibid_sheet
                .write_string(row, 0, &entry.lot_number)
                .map_err(|e| e.to_string())?;
            hibid_sheet
                .write_string(row, 1, &entry.title)
                .map_err(|e| e.to_string())?;
            hibid_sheet
                .write_string(row, 2, &entry.season_header)
                .map_err(|e| e.to_string())?;
            hibid_sheet
                .write_string(row, 4, &entry.bidder_id)
                .map_err(|e| e.to_string())?;
            hibid_sheet
                .write_string(row, 5, &entry.buyer_name)
                .map_err(|e| e.to_string())?;
            hibid_sheet
                .write_string(row, 9, &entry.bidder_email)
                .map_err(|e| e.to_string())?;
            hibid_sheet
                .write_string(row, 10, &entry.bidder_phone)
                .map_err(|e| e.to_string())?;
            hibid_sheet
                .write_number(row, 12, entry.high_bid_cents)
                .map_err(|e| e.to_string())?;
            hibid_sheet
                .write_number(row, 13, entry.max_bid_cents)
                .map_err(|e| e.to_string())?;
        }
        hibid_sheet
            .autofilter(0, 0, hibid_stat_rows.len() as u32, 16)
            .map_err(|e| e.to_string())?;

        workbook
            .save(file_path)
            .map_err(|e| format!("Failed to save detail report: {}", e))?;
        Ok(())
    }

    fn generate_summary_report(
        auction_name: &str,
        items: &[ReportItem],
        file_path: &str,
    ) -> std::result::Result<(), String> {
        let mut workbook = Workbook::new();
        let worksheet = workbook.add_worksheet();
        worksheet
            .set_name(auction_name.get(..31).unwrap_or(auction_name))
            .map_err(|e| e.to_string())?;

        // Formats
        let title_format = Format::new().set_bold().set_font_size(14);

        let header_format = Format::new()
            .set_bold()
            .set_border(FormatBorder::Thin)
            .set_background_color(Color::RGB(0xD9E1F2))
            .set_align(FormatAlign::Center);

        let bold_format = Format::new().set_bold().set_border(FormatBorder::Thin);

        let data_format = Format::new().set_border(FormatBorder::Thin);

        let number_format = Format::new()
            .set_border(FormatBorder::Thin)
            .set_num_format("#,##0");

        let percent_format = Format::new()
            .set_border(FormatBorder::Thin)
            .set_num_format("0%");

        let currency_format = Format::new()
            .set_border(FormatBorder::Thin)
            .set_num_format("#,##0.00");

        // Set column widths
        worksheet
            .set_column_width(1, 25)
            .map_err(|e| e.to_string())?; // Col B (Categories)
        for col in 2..=11 {
            worksheet
                .set_column_width(col, 15)
                .map_err(|e| e.to_string())?; // Cols C-L
        }

        // Title Row
        let title = format!("Отчет по продажам товаров на аукционе ({})", auction_name);
        worksheet
            .write_string_with_format(1, 1, &title, &title_format)
            .map_err(|e| e.to_string())?;

        // Calculate totals
        let total_count = items.len();
        let total_retail: f64 = items.iter().map(|i| i.retail_price).sum();
        let total_cost: f64 = items.iter().map(|i| i.cost_price).sum();

        let buyback_items: Vec<&ReportItem> = items.iter().filter(|i| i.is_buyback).collect();
        let buyback_count = buyback_items.len();
        let buyback_retail: f64 = buyback_items.iter().map(|i| i.retail_price).sum();
        let buyback_cost: f64 = buyback_items.iter().map(|i| i.cost_price).sum();

        let unsold_items: Vec<&ReportItem> = items
            .iter()
            .filter(|i| !i.is_buyback && i.status == "Unsold")
            .collect();
        let unsold_count = unsold_items.len();
        let unsold_retail: f64 = unsold_items.iter().map(|i| i.retail_price).sum();
        let unsold_cost: f64 = unsold_items.iter().map(|i| i.cost_price).sum();

        let sold_items: Vec<&ReportItem> = items.iter().filter(|i| i.status == "Sold").collect();
        let sold_count = sold_items.len();
        let sold_retail: f64 = sold_items.iter().map(|i| i.retail_price).sum();
        let sold_cost: f64 = sold_items.iter().map(|i| i.cost_price).sum();
        let sold_sales: f64 = sold_items.iter().map(|i| i.selling_price).sum();
        let sold_with_commission = sold_sales * (1.0 + REPORT_BONUS_RATE);
        let sold_profit = sold_with_commission - sold_cost;

        // Group sold items by source
        let mut source_groups: HashMap<String, Vec<&ReportItem>> = HashMap::new();
        for item in &sold_items {
            source_groups
                .entry(item.source.clone())
                .or_default()
                .push(item);
        }
        let mut sources: Vec<String> = source_groups.keys().cloned().collect();
        sources.sort();

        // Group buyback items by source
        let mut buyback_groups: HashMap<String, Vec<&ReportItem>> = HashMap::new();
        for item in &buyback_items {
            buyback_groups
                .entry(item.source.clone())
                .or_default()
                .push(item);
        }
        let mut bb_sources: Vec<String> = buyback_groups.keys().cloned().collect();
        bb_sources.sort();

        // ============================================
        // SECTION 1: General Info
        // ============================================
        let section1_headers = [
            "Категории",
            "Кол-во",
            "ритейл цена",
            "себестоимость",
            "% себес.",
            "Дальнейшие действия",
        ];
        for (col, h) in section1_headers.iter().enumerate() {
            worksheet
                .write_string_with_format(2, (col + 1) as u16, *h, &header_format)
                .map_err(|e| e.to_string())?;
        }

        let mut row: u32 = 3;

        // Total
        worksheet
            .write_string_with_format(row, 1, "Всего", &bold_format)
            .map_err(|e| e.to_string())?;
        worksheet
            .write_number_with_format(row, 2, total_count as f64, &number_format)
            .map_err(|e| e.to_string())?;
        worksheet
            .write_number_with_format(row, 3, total_retail, &number_format)
            .map_err(|e| e.to_string())?;
        worksheet
            .write_number_with_format(row, 4, total_cost, &number_format)
            .map_err(|e| e.to_string())?;
        let total_cost_pct = if total_retail > 0.0 {
            total_cost / total_retail
        } else {
            0.0
        };
        worksheet
            .write_number_with_format(row, 5, total_cost_pct, &percent_format)
            .map_err(|e| e.to_string())?;
        row += 1;

        // Buyback
        worksheet
            .write_string_with_format(row, 1, "Выкуплено обратно", &bold_format)
            .map_err(|e| e.to_string())?;
        worksheet
            .write_number_with_format(row, 2, buyback_count as f64, &number_format)
            .map_err(|e| e.to_string())?;
        worksheet
            .write_number_with_format(row, 3, buyback_retail, &number_format)
            .map_err(|e| e.to_string())?;
        worksheet
            .write_number_with_format(row, 4, buyback_cost, &number_format)
            .map_err(|e| e.to_string())?;
        let bb_cost_pct = if buyback_retail > 0.0 {
            buyback_cost / buyback_retail
        } else {
            0.0
        };
        worksheet
            .write_number_with_format(row, 5, bb_cost_pct, &percent_format)
            .map_err(|e| e.to_string())?;
        row += 1;

        // Buyback source breakdown
        for source in &bb_sources {
            let group = &buyback_groups[source];
            let g_count = group.len();
            let g_retail: f64 = group.iter().map(|i| i.retail_price).sum();
            let g_cost: f64 = group.iter().map(|i| i.cost_price).sum();
            let g_cost_pct = if g_retail > 0.0 {
                g_cost / g_retail
            } else {
                0.0
            };

            worksheet
                .write_string_with_format(row, 1, source, &data_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(row, 2, g_count as f64, &number_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(row, 3, g_retail, &number_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(row, 4, g_cost, &number_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(row, 5, g_cost_pct, &percent_format)
                .map_err(|e| e.to_string())?;
            row += 1;
        }

        // Unsold
        worksheet
            .write_string_with_format(row, 1, "Не продано", &data_format)
            .map_err(|e| e.to_string())?;
        worksheet
            .write_number_with_format(row, 2, unsold_count as f64, &number_format)
            .map_err(|e| e.to_string())?;
        worksheet
            .write_number_with_format(row, 3, unsold_retail, &number_format)
            .map_err(|e| e.to_string())?;
        worksheet
            .write_number_with_format(row, 4, unsold_cost, &number_format)
            .map_err(|e| e.to_string())?;
        row += 2; // Extra row gap

        // ============================================
        // SECTION 2: Sold on Auction
        // ============================================
        worksheet
            .write_string_with_format(row, 1, "Проданные на аукционе товары", &bold_format)
            .map_err(|e| e.to_string())?;
        row += 1;
        let section2_headers = [
            "Категории",
            "Кол-во",
            "ритейл цена",
            "себестоимость",
            "% себес.",
            "Продажи",
            "с ком 15%",
            "% продажи",
            "прибыль/убыток",
        ];
        for (col, h) in section2_headers.iter().enumerate() {
            worksheet
                .write_string_with_format(row, (col + 1) as u16, *h, &header_format)
                .map_err(|e| e.to_string())?;
        }
        worksheet
            .write_string_with_format(row, 10, "% прибыли", &header_format)
            .map_err(|e| e.to_string())?; // Col K
        row += 1;

        // Sold breakdown by source
        let sold_row = row;
        row += 1;
        let vendor_start_row = row;
        for source in &sources {
            let group = &source_groups[source];
            let g_count = group.len();
            let g_retail: f64 = group.iter().map(|i| i.retail_price).sum();
            let g_cost: f64 = group.iter().map(|i| i.cost_price).sum();
            let g_cost_pct = if g_retail > 0.0 {
                g_cost / g_retail
            } else {
                0.0
            };
            let g_sales: f64 = group.iter().map(|i| i.selling_price).sum();
            let g_with_comm = g_sales * (1.0 + REPORT_BONUS_RATE);
            let g_sales_pct = if g_retail > 0.0 {
                g_with_comm / g_retail
            } else {
                0.0
            };
            let g_profit = g_with_comm - g_cost;
            let g_profit_pct = if sold_profit != 0.0 {
                g_profit / sold_profit.abs()
            } else {
                0.0
            };

            worksheet
                .write_string_with_format(row, 1, source, &data_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(row, 2, g_count as f64, &number_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(row, 3, g_retail, &number_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(row, 4, g_cost, &number_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(row, 5, g_cost_pct, &percent_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(row, 6, g_sales, &number_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(row, 7, g_with_comm, &number_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(row, 8, g_sales_pct, &percent_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(row, 9, g_profit, &currency_format)
                .map_err(|e| e.to_string())?;
            if g_profit > 0.0 {
                worksheet
                    .write_number_with_format(row, 10, g_profit_pct, &percent_format)
                    .map_err(|e| e.to_string())?;
            }
            row += 1;
        }
        let vendor_end_row = row.saturating_sub(1);

        // Sold Total (formula-driven so spreadsheet remains live)
        worksheet
            .write_string_with_format(sold_row, 1, "Продано", &bold_format)
            .map_err(|e| e.to_string())?;
        if vendor_start_row <= vendor_end_row {
            worksheet
                .write_formula_with_format(
                    sold_row,
                    2,
                    format!("=SUM(C{}:C{})", vendor_start_row + 1, vendor_end_row + 1).as_str(),
                    &number_format,
                )
                .map_err(|e| e.to_string())?;
            worksheet
                .write_formula_with_format(
                    sold_row,
                    3,
                    format!("=SUM(D{}:D{})", vendor_start_row + 1, vendor_end_row + 1).as_str(),
                    &number_format,
                )
                .map_err(|e| e.to_string())?;
            worksheet
                .write_formula_with_format(
                    sold_row,
                    4,
                    format!("=SUM(E{}:E{})", vendor_start_row + 1, vendor_end_row + 1).as_str(),
                    &number_format,
                )
                .map_err(|e| e.to_string())?;
            worksheet
                .write_formula_with_format(
                    sold_row,
                    6,
                    format!("=SUM(G{}:G{})", vendor_start_row + 1, vendor_end_row + 1).as_str(),
                    &number_format,
                )
                .map_err(|e| e.to_string())?;
        } else {
            worksheet
                .write_number_with_format(sold_row, 2, sold_count as f64, &number_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(sold_row, 3, sold_retail, &number_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(sold_row, 4, sold_cost, &number_format)
                .map_err(|e| e.to_string())?;
            worksheet
                .write_number_with_format(sold_row, 6, sold_sales, &number_format)
                .map_err(|e| e.to_string())?;
        }
        worksheet
            .write_formula_with_format(
                sold_row,
                5,
                format!(
                    "=IF(D{}=0,0,E{}/D{})",
                    sold_row + 1,
                    sold_row + 1,
                    sold_row + 1
                )
                .as_str(),
                &percent_format,
            )
            .map_err(|e| e.to_string())?;
        worksheet
            .write_formula_with_format(
                sold_row,
                7,
                format!("=G{}*{:.2}", sold_row + 1, 1.0 + REPORT_BONUS_RATE).as_str(),
                &number_format,
            )
            .map_err(|e| e.to_string())?;
        worksheet
            .write_formula_with_format(
                sold_row,
                8,
                format!(
                    "=IF(D{}=0,0,H{}/D{})",
                    sold_row + 1,
                    sold_row + 1,
                    sold_row + 1
                )
                .as_str(),
                &percent_format,
            )
            .map_err(|e| e.to_string())?;
        worksheet
            .write_formula_with_format(
                sold_row,
                9,
                format!("=H{}-E{}", sold_row + 1, sold_row + 1).as_str(),
                &currency_format,
            )
            .map_err(|e| e.to_string())?;

        row += 2; // Extra row gap

        // ============================================
        // SECTION 3: Cash Sales
        // ============================================
        worksheet
            .write_string_with_format(row, 1, "Продажа наличными", &bold_format)
            .map_err(|e| e.to_string())?;
        row += 1;
        let section3_headers = [
            "Категории",
            "кол-во",
            "ритейл цена",
            "себест.",
            "% себес.",
            "Продажи",
            "Клейт 11%",
            "% продаж",
            "прибыль/убыток",
        ];
        for (col, h) in section3_headers.iter().enumerate() {
            worksheet
                .write_string_with_format(row, (col + 1) as u16, *h, &header_format)
                .map_err(|e| e.to_string())?;
        }
        row += 1;

        // Blank row for user to fill in if needed
        let cash_sales_row = row;
        worksheet
            .write_string_with_format(row, 1, "", &data_format)
            .map_err(|e| e.to_string())?;

        row += 2; // Extra row gap

        // ============================================
        // SECTION 4: Overall Total
        // ============================================
        worksheet
            .write_string_with_format(row, 1, "Общие продажи", &bold_format)
            .map_err(|e| e.to_string())?;
        worksheet
            .write_formula_with_format(
                row,
                9,
                format!("=SUM(J{},J{})", sold_row + 1, cash_sales_row + 1).as_str(),
                &bold_format,
            )
            .map_err(|e| e.to_string())?; // Col J (Profit column)

        workbook
            .save(file_path)
            .map_err(|e| format!("Failed to save summary report: {}", e))?;
        Ok(())
    }

    pub fn get_auction_reports(
        db: &Database,
        auction_id: &str,
    ) -> std::result::Result<Vec<AuctionReport>, String> {
        let mut stmt = db.conn.prepare(
            "SELECT r.id, r.auction_id, a.name, r.report_type, r.file_name, r.file_path, r.created_at
             FROM auction_reports r
             JOIN auctions a ON a.id = r.auction_id
             WHERE r.auction_id = ?1
             ORDER BY r.created_at DESC"
        ).map_err(|e| e.to_string())?;

        let reports = stmt
            .query_map(rusqlite::params![auction_id], |row| {
                Ok(AuctionReport {
                    id: row.get(0)?,
                    auction_id: row.get(1)?,
                    auction_name: row.get(2)?,
                    report_type: row.get(3)?,
                    file_name: row.get(4)?,
                    file_path: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;

        Ok(reports)
    }

    pub fn get_all_auction_reports(
        db: &Database,
    ) -> std::result::Result<Vec<AuctionReport>, String> {
        let mut stmt = db.conn.prepare(
            "SELECT r.id, r.auction_id, a.name, r.report_type, r.file_name, r.file_path, r.created_at
             FROM auction_reports r
             JOIN auctions a ON a.id = r.auction_id
             ORDER BY r.created_at DESC"
        ).map_err(|e| e.to_string())?;

        let reports = stmt
            .query_map([], |row| {
                Ok(AuctionReport {
                    id: row.get(0)?,
                    auction_id: row.get(1)?,
                    auction_name: row.get(2)?,
                    report_type: row.get(3)?,
                    file_name: row.get(4)?,
                    file_path: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;

        Ok(reports)
    }
}

// Tauri Commands

#[tauri::command]
pub fn create_auction(
    req: CreateAuctionRequest,
    state: State<crate::AppState>,
) -> std::result::Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    AuctionManager::create_auction(&db, req).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_auctions(state: State<crate::AppState>) -> std::result::Result<Vec<Auction>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    AuctionManager::list_auctions(&db).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_auction_by_id(
    auction_id: String,
    state: State<crate::AppState>,
) -> std::result::Result<Auction, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    AuctionManager::get_auction_by_id(&db, &auction_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_auction_status(
    auction_id: String,
    status: String,
    state: State<crate::AppState>,
) -> std::result::Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    AuctionManager::update_auction_status(&db, &auction_id, &status).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_vendor(
    vendor_id: String,
    data: UpdateVendorRequest,
    state: State<crate::AppState>,
) -> std::result::Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    AuctionManager::update_vendor(&db, &vendor_id, &data).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn unassign_item(
    item_id: String,
    state: State<crate::AppState>,
) -> std::result::Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Get auction_id before reset
    let auction_id: Option<String> = db
        .conn
        .query_row(
            "SELECT auction_id FROM inventory_items WHERE id = ?1",
            rusqlite::params![item_id],
            |r| r.get(0),
        )
        .unwrap_or(None);

    // Reset status and auction_id
    db.conn.execute(
        "UPDATE inventory_items SET current_status = 'InStock', auction_id = NULL, listed_at = NULL WHERE id = ?1",
        rusqlite::params![item_id],
    ).map_err(|e| e.to_string())?;

    // Update auction total_lots count
    if let Some(auc_id) = auction_id {
        db.conn
            .execute(
                "UPDATE auctions SET total_lots = (
                SELECT COUNT(*) FROM inventory_items WHERE auction_id = ?1
             ) WHERE id = ?1",
                rusqlite::params![auc_id],
            )
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn finish_auction(
    auction_id: String,
    results_csv_path: String,
    app_handle: tauri::AppHandle,
    state: State<crate::AppState>,
) -> std::result::Result<FinishAuctionResult, String> {
    use tauri::Manager;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Get app data directory (Tauri 2 API)
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?
        .to_string_lossy()
        .to_string();

    AuctionManager::finish_auction(&db, &auction_id, &results_csv_path, &app_data_dir)
}

#[tauri::command]
pub fn get_auction_reports(
    auction_id: String,
    state: State<crate::AppState>,
) -> std::result::Result<Vec<AuctionReport>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    AuctionManager::get_auction_reports(&db, &auction_id)
}

#[tauri::command]
pub fn get_all_auction_reports(
    state: State<crate::AppState>,
) -> std::result::Result<Vec<AuctionReport>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    AuctionManager::get_all_auction_reports(&db)
}

#[tauri::command]
pub fn rename_auction(
    auction_id: String,
    name: String,
    state: State<crate::AppState>,
) -> std::result::Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let normalized_name = normalize_auction_name(&name)
        .ok_or_else(|| "Invalid auction number. Use positive number.".to_string())?;
    db.conn
        .execute(
            "UPDATE auctions SET name = ?1 WHERE id = ?2",
            rusqlite::params![normalized_name, auction_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_auction_result_bids(
    auction_id: String,
    state: State<crate::AppState>,
) -> std::result::Result<Vec<AuctionResultBid>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db
        .conn
        .prepare(
            "SELECT item_id, high_bid
             FROM auction_results
             WHERE auction_id = ?1",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(rusqlite::params![auction_id], |row| {
            Ok(AuctionResultBid {
                item_id: row.get(0)?,
                high_bid: row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_relistable_inventory_items(
    auction_id: String,
    state: State<crate::AppState>,
) -> std::result::Result<Vec<crate::db::InventoryItemRow>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let items = db.get_inventory_items(None).map_err(|e| e.to_string())?;
    let filtered = items
        .into_iter()
        .filter(|item| {
            matches!(
                item.current_status.as_str(),
                "InStock" | "Unsold" | "Buyback" | "FloorSale"
            ) && item.auction_id.as_deref() != Some(auction_id.as_str())
        })
        .collect::<Vec<_>>();
    Ok(filtered)
}

#[tauri::command]
pub fn assign_items_to_auction(
    auction_id: String,
    item_ids: Vec<String>,
    state: State<crate::AppState>,
) -> std::result::Result<i32, String> {
    if item_ids.is_empty() {
        return Ok(0);
    }

    let mut db = state.db.lock().map_err(|e| e.to_string())?;
    let auction_status: String = db
        .conn
        .query_row(
            "SELECT status FROM auctions WHERE id = ?1",
            rusqlite::params![&auction_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Auction not found: {}", e))?;
    if auction_status != "Active" {
        return Err("Items can only be added to an active auction".to_string());
    }

    let tx = db.conn.transaction().map_err(|e| e.to_string())?;
    let mut affected_count: i32 = 0;
    let mut touched_ids: HashSet<String> = HashSet::new();
    for item_id in item_ids {
        if !touched_ids.insert(item_id.clone()) {
            continue;
        }
        let affected = tx
            .execute(
                "UPDATE inventory_items
                 SET current_status = 'Listed',
                     auction_id = ?1,
                     listed_at = CURRENT_TIMESTAMP,
                     sold_at = NULL,
                     sale_order = NULL,
                     buybacker_id = NULL
                 WHERE id = ?2
                   AND current_status IN ('InStock', 'Unsold', 'Buyback', 'FloorSale')",
                rusqlite::params![&auction_id, &item_id],
            )
            .map_err(|e| e.to_string())?;
        affected_count += affected as i32;
    }

    tx.execute(
        "UPDATE auctions
         SET total_lots = (
             SELECT COUNT(*)
             FROM inventory_items
             WHERE auction_id = ?1
         )
         WHERE id = ?1",
        rusqlite::params![&auction_id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;

    Ok(affected_count)
}

#[tauri::command]
pub fn get_item_repeater_stats(
    normalized_titles: Vec<String>,
    season_headers: Vec<String>,
    state: State<crate::AppState>,
) -> std::result::Result<HashMap<String, HashMap<String, f64>>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let mut titles = normalized_titles
        .into_iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>();
    titles.sort();
    titles.dedup();

    let mut header_by_number: HashMap<u32, String> = HashMap::new();
    let mut valid_headers = Vec::new();
    for header in season_headers {
        if let Some(number) = extract_auction_number(&header) {
            header_by_number.insert(number, header.clone());
            valid_headers.push(header);
        }
    }

    let mut result: HashMap<String, HashMap<String, f64>> = HashMap::new();
    for title in &titles {
        let mut per_header = HashMap::new();
        for header in &valid_headers {
            per_header.insert(header.clone(), 0.0);
        }
        result.insert(title.clone(), per_header);
    }

    if titles.is_empty() || valid_headers.is_empty() {
        return Ok(result);
    }

    let placeholders = vec!["?"; titles.len()].join(",");
    let sql = format!(
        "SELECT i.normalized_title, a.name, MAX(ar.high_bid) as max_bid
         FROM auction_results ar
         JOIN inventory_items i ON i.id = ar.item_id
         JOIN auctions a ON a.id = ar.auction_id
         WHERE i.normalized_title IN ({})
           AND ar.high_bid > 0
           AND (
             ar.item_status IN ('Unsold', 'Buyback')
             OR (
               ar.item_status IS NULL
               AND (
                 ar.is_buyback = 1
                 OR trim(COALESCE(ar.winning_bidder, '')) = ''
               )
             )
           )
         GROUP BY i.normalized_title, a.name",
        placeholders
    );

    let mut stmt = db.conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(titles.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    for row in rows {
        let (title, auction_name, max_bid) = row.map_err(|e| e.to_string())?;
        let Some(auction_number) = extract_auction_number(&auction_name) else {
            continue;
        };
        let Some(header) = header_by_number.get(&auction_number) else {
            continue;
        };
        let title_entry = result.entry(title).or_insert_with(|| {
            valid_headers
                .iter()
                .map(|h| (h.clone(), 0.0))
                .collect::<HashMap<String, f64>>()
        });
        let current = title_entry.get(header).copied().unwrap_or(0.0);
        if max_bid > current {
            title_entry.insert(header.clone(), max_bid);
        }
    }

    Ok(result)
}

#[tauri::command]
pub fn get_item_first_auction_map(
    item_ids: Vec<String>,
    state: State<crate::AppState>,
) -> std::result::Result<HashMap<String, String>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let mut ids = item_ids
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();

    let mut result: HashMap<String, String> = HashMap::new();
    if ids.is_empty() {
        return Ok(result);
    }

    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!(
        "SELECT ia.item_id, a.name, a.created_at
         FROM (
             SELECT DISTINCT ar.item_id AS item_id, ar.auction_id AS auction_id
             FROM auction_results ar
             WHERE ar.item_id IN ({placeholders})
             UNION
             SELECT i.id AS item_id, i.auction_id AS auction_id
             FROM inventory_items i
             WHERE i.id IN ({placeholders})
               AND i.auction_id IS NOT NULL
         ) ia
         JOIN auctions a ON a.id = ia.auction_id"
    );

    let mut params = Vec::with_capacity(ids.len() * 2);
    params.extend(ids.iter().cloned());
    params.extend(ids.iter().cloned());

    let mut stmt = db.conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut earliest_by_item: HashMap<String, (String, String)> = HashMap::new();

    for row in rows {
        let (item_id, auction_name, created_at) = row.map_err(|e| e.to_string())?;
        match earliest_by_item.get(&item_id) {
            Some((known_created_at, known_auction_name)) => {
                let replace = if created_at < *known_created_at {
                    true
                } else if created_at == *known_created_at {
                    match (
                        extract_auction_number(&auction_name),
                        extract_auction_number(known_auction_name),
                    ) {
                        (Some(next), Some(current)) => next < current,
                        (Some(_), None) => true,
                        _ => false,
                    }
                } else {
                    false
                };
                if replace {
                    earliest_by_item.insert(item_id, (created_at, auction_name));
                }
            }
            None => {
                earliest_by_item.insert(item_id, (created_at, auction_name));
            }
        }
    }

    for (item_id, (_, auction_name)) in earliest_by_item {
        result.insert(item_id, auction_name);
    }

    Ok(result)
}

#[tauri::command]
pub fn delete_auction(
    auction_id: String,
    state: State<crate::AppState>,
) -> std::result::Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    // Reset items back to InStock
    db.conn.execute(
        "UPDATE inventory_items SET current_status = 'InStock', auction_id = NULL, listed_at = NULL WHERE auction_id = ?1",
        rusqlite::params![auction_id],
    ).map_err(|e| e.to_string())?;
    // Delete related records
    db.conn
        .execute(
            "DELETE FROM auction_results WHERE auction_id = ?1",
            rusqlite::params![auction_id],
        )
        .map_err(|e| e.to_string())?;
    db.conn
        .execute(
            "DELETE FROM auction_reports WHERE auction_id = ?1",
            rusqlite::params![auction_id],
        )
        .map_err(|e| e.to_string())?;
    // Delete the auction itself
    db.conn
        .execute(
            "DELETE FROM auctions WHERE id = ?1",
            rusqlite::params![auction_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn open_report_file(file_path: String) -> std::result::Result<(), String> {
    // Open file with default system application
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &file_path])
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("Failed to open file: {}", e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
    };

    fn path_str(path: &Path) -> &str {
        path.to_str().expect("Path should be valid UTF-8")
    }

    #[cfg(target_os = "windows")]
    fn extract_sheet_xml(base_dir: &Path, label: &str, xlsx_path: &str) -> String {
        let out_dir = base_dir.join(format!("unzipped_{}", label));
        let _ = fs::remove_dir_all(&out_dir);
        fs::create_dir_all(&out_dir).expect("Failed to create unzip dir");

        let status = Command::new("tar")
            .args(["-xf", xlsx_path, "-C", path_str(&out_dir)])
            .status()
            .expect("Failed to run tar for xlsx extraction");
        assert!(status.success(), "tar extraction failed for {}", xlsx_path);

        fs::read_to_string(out_dir.join("xl").join("worksheets").join("sheet1.xml"))
            .expect("Failed to read worksheet xml")
    }

    #[test]
    fn finish_auction_generates_report_with_cached_results() {
        let base_dir: PathBuf =
            std::env::temp_dir().join(format!("sugarland_finish_auction_smoke_{}", Uuid::new_v4()));
        fs::create_dir_all(&base_dir).expect("Failed to create temp dir");

        let db_path = base_dir.join("smoke.db");
        let db = Database::new(path_str(&db_path)).expect("Failed to create test db");

        let manifest_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO manifests (id, source_filename, total_retail_value, total_cost, items_count)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![manifest_id, "manifest.csv", 1000.0_f64, 140.0_f64, 1_i64],
            )
            .expect("Failed to insert manifest");

        let auction_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO auctions (id, name, status, total_lots)
                 VALUES (?1, ?2, 'Active', 1)",
                params![auction_id, "Sugarland 999"],
            )
            .expect("Failed to insert auction");

        let item_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO inventory_items (
                    id, manifest_id, lot_number, quantity, raw_title, vendor_code, source,
                    retail_price, cost_price, min_price, current_status, auction_id
                 ) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, ?8, ?9, 'Listed', ?10)",
                params![
                    item_id,
                    manifest_id,
                    "27m",
                    "Thermador Dishwasher",
                    "BB-001",
                    "Best Buy",
                    1000.0_f64,
                    140.0_f64,
                    100.0_f64,
                    auction_id
                ],
            )
            .expect("Failed to insert inventory item");

        let csv_path = base_dir.join("results.csv");
        fs::write(
            &csv_path,
            "Lot,Title,Winning Bidder,Name,High Bid,Max Bid,Email,Phone\n27m,Thermador Dishwasher,1001,Test Buyer,30500,,,\n",
        )
        .expect("Failed to write csv");

        let app_data_dir = base_dir.join("app_data");
        fs::create_dir_all(&app_data_dir).expect("Failed to create app data dir");

        let reports = AuctionManager::finish_auction(
            &db,
            &auction_id,
            path_str(&csv_path),
            path_str(&app_data_dir),
        )
        .expect("finish_auction failed");

        let reports_dir = app_data_dir.join("reports").join(&auction_id);
        let detail_path = reports_dir.join(&reports.detail_report);
        let summary_path = reports_dir.join(&reports.summary_report);

        assert!(detail_path.exists(), "detail report missing");
        assert!(summary_path.exists(), "summary report missing");

        #[cfg(target_os = "windows")]
        {
            let detail_xml = extract_sheet_xml(&base_dir, "detail", path_str(&detail_path));
            assert!(
                !detail_xml.contains("IFERROR(VLOOKUP("),
                "Detail report should not rely on sale VLOOKUP formulas"
            );
            assert!(
                detail_xml.contains("<c r=\"N2\"") && detail_xml.contains("<v>305</v>"),
                "Detail report missing cached sale value"
            );
            assert!(
                detail_xml.contains("<f>N2-I2</f>"),
                "Detail report missing difference formula"
            );
            assert!(
                detail_xml.contains("<f>N2*0.11+P2</f>"),
                "Detail report missing plus bonus formula"
            );
            assert!(
                detail_xml.contains("MAXIFS('Hibid stat'!$M:$M")
                    || detail_xml.contains("_xlfn.MAXIFS('Hibid stat'!$M:$M"),
                "Detail report missing historical MAXIFS formula"
            );

            let summary_xml = extract_sheet_xml(&base_dir, "summary", path_str(&summary_path));
            assert!(
                summary_xml.contains("<f>SUM(D"),
                "Summary report missing sold retail SUM formula"
            );
            assert!(
                summary_xml.contains("<f>G") && summary_xml.contains("*1.11"),
                "Summary report missing sold commission formula"
            );
            assert!(
                summary_xml.contains("<f>SUM(J"),
                "Summary report missing overall total formula"
            );
        }
    }

    #[test]
    fn finish_auction_allows_completed_status_for_regeneration() {
        let base_dir: PathBuf = std::env::temp_dir().join(format!(
            "sugarland_finish_auction_regenerate_{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&base_dir).expect("Failed to create temp dir");

        let db_path = base_dir.join("regenerate.db");
        let db = Database::new(path_str(&db_path)).expect("Failed to create test db");

        let manifest_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO manifests (id, source_filename, total_retail_value, total_cost, items_count)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![manifest_id, "manifest.csv", 1000.0_f64, 140.0_f64, 1_i64],
            )
            .expect("Failed to insert manifest");

        let auction_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO auctions (id, name, status, total_lots)
                 VALUES (?1, ?2, 'Completed', 1)",
                params![auction_id, "Sugarland 1002"],
            )
            .expect("Failed to insert auction");

        let item_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO inventory_items (
                    id, manifest_id, lot_number, quantity, raw_title, vendor_code, source,
                    retail_price, cost_price, min_price, current_status, auction_id
                 ) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, ?8, ?9, 'Listed', ?10)",
                params![
                    item_id,
                    manifest_id,
                    "88m",
                    "Regeneration Candidate",
                    "BB-088",
                    "Best Buy",
                    1000.0_f64,
                    140.0_f64,
                    100.0_f64,
                    auction_id
                ],
            )
            .expect("Failed to insert inventory item");

        let csv_path = base_dir.join("results_regenerate.csv");
        fs::write(
            &csv_path,
            "Lot,Title,Winning Bidder,Name,High Bid,Max Bid,Email,Phone\n88m,Regeneration Candidate,7001,Retry Buyer,9900,,,\n",
        )
        .expect("Failed to write csv");

        let app_data_dir = base_dir.join("app_data");
        fs::create_dir_all(&app_data_dir).expect("Failed to create app data dir");

        let result = AuctionManager::finish_auction(
            &db,
            &auction_id,
            path_str(&csv_path),
            path_str(&app_data_dir),
        );
        assert!(result.is_ok(), "Completed auctions must allow regeneration");
    }

    #[test]
    fn finish_auction_uses_fallback_buyer_when_name_missing() {
        let base_dir: PathBuf = std::env::temp_dir().join(format!(
            "sugarland_finish_auction_fallback_buyer_{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&base_dir).expect("Failed to create temp dir");

        let db_path = base_dir.join("fallback_buyer.db");
        let db = Database::new(path_str(&db_path)).expect("Failed to create test db");

        let manifest_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO manifests (id, source_filename, total_retail_value, total_cost, items_count)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![manifest_id, "manifest.csv", 1200.0_f64, 180.0_f64, 1_i64],
            )
            .expect("Failed to insert manifest");

        let auction_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO auctions (id, name, status, total_lots)
                 VALUES (?1, ?2, 'Active', 1)",
                params![auction_id, "Sugarland 1003"],
            )
            .expect("Failed to insert auction");

        let item_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO inventory_items (
                    id, manifest_id, lot_number, quantity, raw_title, vendor_code, source,
                    retail_price, cost_price, min_price, current_status, auction_id
                 ) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, ?8, ?9, 'Listed', ?10)",
                params![
                    item_id,
                    manifest_id,
                    "99m",
                    "Fallback Buyer Lot",
                    "BB-099",
                    "Best Buy",
                    1200.0_f64,
                    180.0_f64,
                    100.0_f64,
                    auction_id
                ],
            )
            .expect("Failed to insert inventory item");

        let csv_path = base_dir.join("results_fallback_buyer.csv");
        fs::write(
            &csv_path,
            "Lot,Title,Winning Bidder,Name,High Bid,Max Bid,Email,Phone\n99m,Fallback Buyer Lot,8123,,24500,,,\n",
        )
        .expect("Failed to write csv");

        let app_data_dir = base_dir.join("app_data");
        fs::create_dir_all(&app_data_dir).expect("Failed to create app data dir");

        AuctionManager::finish_auction(
            &db,
            &auction_id,
            path_str(&csv_path),
            path_str(&app_data_dir),
        )
        .expect("finish_auction failed");

        let (buyer, status, high_bid): (String, String, f64) = db
            .conn
            .query_row(
                "SELECT winning_bidder, item_status, high_bid
                 FROM auction_results
                 WHERE auction_id = ?1 AND item_id = ?2",
                params![auction_id, item_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("Failed to fetch auction result");

        assert_eq!(buyer, "8123");
        assert_eq!(status, "Sold");
        assert!((high_bid - 245.0).abs() < 0.001);
    }

    #[test]
    fn finish_auction_keeps_floor_label_for_unsold_rows() {
        let base_dir: PathBuf = std::env::temp_dir().join(format!(
            "sugarland_finish_auction_floor_label_{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&base_dir).expect("Failed to create temp dir");

        let db_path = base_dir.join("floor_label.db");
        let db = Database::new(path_str(&db_path)).expect("Failed to create test db");

        let manifest_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO manifests (id, source_filename, total_retail_value, total_cost, items_count)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![manifest_id, "manifest.csv", 600.0_f64, 90.0_f64, 1_i64],
            )
            .expect("Failed to insert manifest");

        let auction_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO auctions (id, name, status, total_lots)
                 VALUES (?1, ?2, 'Active', 1)",
                params![auction_id, "Sugarland 1004"],
            )
            .expect("Failed to insert auction");

        let item_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO inventory_items (
                    id, manifest_id, lot_number, quantity, raw_title, vendor_code, source,
                    retail_price, cost_price, min_price, current_status, auction_id
                 ) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, ?8, ?9, 'Listed', ?10)",
                params![
                    item_id,
                    manifest_id,
                    "122",
                    "Floor Lot",
                    "BB-122",
                    "Best Buy",
                    600.0_f64,
                    90.0_f64,
                    80.0_f64,
                    auction_id
                ],
            )
            .expect("Failed to insert inventory item");

        let csv_path = base_dir.join("results_floor.csv");
        fs::write(
            &csv_path,
            "Lot,Title,Winning Bidder,Name,High Bid,Max Bid,Email,Phone\n122,Floor Lot,,Floor,000,0,,,\n",
        )
        .expect("Failed to write csv");

        let app_data_dir = base_dir.join("app_data");
        fs::create_dir_all(&app_data_dir).expect("Failed to create app data dir");

        AuctionManager::finish_auction(
            &db,
            &auction_id,
            path_str(&csv_path),
            path_str(&app_data_dir),
        )
        .expect("finish_auction failed");

        let (buyer, status, high_bid): (String, String, f64) = db
            .conn
            .query_row(
                "SELECT winning_bidder, item_status, high_bid
                 FROM auction_results
                 WHERE auction_id = ?1 AND item_id = ?2",
                params![auction_id, item_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("Failed to fetch auction result");

        assert_eq!(buyer, "Floor");
        assert_eq!(status, "Unsold");
        assert!(high_bid.abs() < 0.001);
    }

    #[test]
    fn finish_auction_marks_buyback_for_trimmed_active_name() {
        let base_dir: PathBuf = std::env::temp_dir().join(format!(
            "sugarland_finish_auction_buyback_{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&base_dir).expect("Failed to create temp dir");

        let db_path = base_dir.join("buyback.db");
        let db = Database::new(path_str(&db_path)).expect("Failed to create test db");

        let manifest_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO manifests (id, source_filename, total_retail_value, total_cost, items_count)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![manifest_id, "manifest.csv", 1200.0_f64, 180.0_f64, 1_i64],
            )
            .expect("Failed to insert manifest");

        let auction_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO auctions (id, name, status, total_lots)
                 VALUES (?1, ?2, 'Active', 1)",
                params![auction_id, "Sugarland 1000"],
            )
            .expect("Failed to insert auction");

        db.conn
            .execute(
                "INSERT INTO buybackers (id, name, is_active) VALUES (?1, ?2, 1)",
                params![Uuid::new_v4().to_string(), "ron larson "],
            )
            .expect("Failed to insert buybacker");

        let item_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO inventory_items (
                    id, manifest_id, lot_number, quantity, raw_title, vendor_code, source,
                    retail_price, cost_price, min_price, current_status, auction_id
                 ) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, ?8, ?9, 'Listed', ?10)",
                params![
                    item_id,
                    manifest_id,
                    "42m",
                    "Buyback Candidate",
                    "BB-042",
                    "Best Buy",
                    1200.0_f64,
                    180.0_f64,
                    100.0_f64,
                    auction_id
                ],
            )
            .expect("Failed to insert inventory item");

        let csv_path = base_dir.join("results_buyback.csv");
        fs::write(
            &csv_path,
            "Lot,Title,Winning Bidder,Name,High Bid,Max Bid,Email,Phone\n42m,Buyback Candidate,2002,Ron Larson,30500,,,\n",
        )
        .expect("Failed to write csv");

        let app_data_dir = base_dir.join("app_data");
        fs::create_dir_all(&app_data_dir).expect("Failed to create app data dir");

        AuctionManager::finish_auction(
            &db,
            &auction_id,
            path_str(&csv_path),
            path_str(&app_data_dir),
        )
        .expect("finish_auction failed");

        let status: String = db
            .conn
            .query_row(
                "SELECT item_status FROM auction_results WHERE auction_id = ?1 AND item_id = ?2",
                params![auction_id, item_id],
                |row| row.get(0),
            )
            .expect("Failed to fetch auction result status");

        assert_eq!(status, "Buyback");
    }

    #[test]
    fn finish_auction_marks_below_min_non_buyback_as_sold() {
        let base_dir: PathBuf =
            std::env::temp_dir().join(format!("sugarland_finish_auction_sold_{}", Uuid::new_v4()));
        fs::create_dir_all(&base_dir).expect("Failed to create temp dir");

        let db_path = base_dir.join("sold.db");
        let db = Database::new(path_str(&db_path)).expect("Failed to create test db");

        let manifest_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO manifests (id, source_filename, total_retail_value, total_cost, items_count)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![manifest_id, "manifest.csv", 1200.0_f64, 180.0_f64, 1_i64],
            )
            .expect("Failed to insert manifest");

        let auction_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO auctions (id, name, status, total_lots)
                 VALUES (?1, ?2, 'Active', 1)",
                params![auction_id, "Sugarland 1001"],
            )
            .expect("Failed to insert auction");

        let item_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO inventory_items (
                    id, manifest_id, lot_number, quantity, raw_title, vendor_code, source,
                    retail_price, cost_price, min_price, current_status, auction_id
                 ) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, ?8, ?9, 'Listed', ?10)",
                params![
                    item_id,
                    manifest_id,
                    "77m",
                    "Regular Buyer Lot",
                    "BB-077",
                    "Best Buy",
                    1200.0_f64,
                    180.0_f64,
                    300.0_f64, // Above final bid by design
                    auction_id
                ],
            )
            .expect("Failed to insert inventory item");

        let csv_path = base_dir.join("results_sold.csv");
        fs::write(
            &csv_path,
            "Lot,Title,Winning Bidder,Name,High Bid,Max Bid,Email,Phone\n77m,Regular Buyer Lot,,John Buyer,5000,,,\n",
        )
        .expect("Failed to write csv");

        let app_data_dir = base_dir.join("app_data");
        fs::create_dir_all(&app_data_dir).expect("Failed to create app data dir");

        AuctionManager::finish_auction(
            &db,
            &auction_id,
            path_str(&csv_path),
            path_str(&app_data_dir),
        )
        .expect("finish_auction failed");

        let (status, commission, net_profit): (String, f64, f64) = db
            .conn
            .query_row(
                "SELECT item_status, commission_amount, net_profit
                 FROM auction_results
                 WHERE auction_id = ?1 AND item_id = ?2",
                params![auction_id, item_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("Failed to fetch auction result");

        assert_eq!(status, "Sold");
        assert!((commission - 7.5).abs() < 0.001);
        assert!((net_profit - (50.0 - 180.0)).abs() < 0.001);
    }

    fn make_report_item(item_id: &str, lot_number: &str) -> ReportItem {
        ReportItem {
            item_id: item_id.to_string(),
            lot_number: lot_number.to_string(),
            sale_order: None,
            title: String::new(),
            retail_price: 0.0,
            source: String::new(),
            condition: String::new(),
            cost_coefficient: 0.0,
            min_price_pct: 0.0,
            cost_price: 0.0,
            high_bid: 0.0,
            max_bid: 0.0,
            selling_price: 0.0,
            bidder_id: String::new(),
            buyer: String::new(),
            bidder_email: String::new(),
            bidder_phone: String::new(),
            is_buyback: false,
            status: String::new(),
        }
    }

    #[test]
    fn natural_lot_cmp_orders_lots_with_suffixes() {
        let mut lots = vec![
            "102m".to_string(),
            "10".to_string(),
            "3m".to_string(),
            "3".to_string(),
            "2".to_string(),
            "11".to_string(),
            "102".to_string(),
            "4m".to_string(),
            "4".to_string(),
        ];
        lots.sort_by(|a, b| natural_lot_cmp(a, b));
        assert_eq!(
            lots,
            vec![
                "2".to_string(),
                "3".to_string(),
                "3m".to_string(),
                "4".to_string(),
                "4m".to_string(),
                "10".to_string(),
                "11".to_string(),
                "102".to_string(),
                "102m".to_string(),
            ]
        );
    }

    #[test]
    fn build_sale_order_index_uses_natural_lot_order() {
        let items = vec![
            make_report_item("i1", "10"),
            make_report_item("i2", "2"),
            make_report_item("i3", "3m"),
            make_report_item("i4", "3"),
            make_report_item("i5", "102m"),
            make_report_item("i6", "102"),
        ];
        let index = build_sale_order_index(&items);

        assert_eq!(index.get("i2"), Some(&1));
        assert_eq!(index.get("i4"), Some(&2));
        assert_eq!(index.get("i3"), Some(&3));
        assert_eq!(index.get("i1"), Some(&4));
        assert_eq!(index.get("i6"), Some(&5));
        assert_eq!(index.get("i5"), Some(&6));
    }

    #[test]
    fn finish_auction_includes_unmatched_hibid_rows_in_detail_report() {
        let base_dir: PathBuf = std::env::temp_dir().join(format!(
            "sugarland_finish_auction_unmatched_hibid_{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&base_dir).expect("Failed to create temp dir");

        let db_path = base_dir.join("unmatched_hibid.db");
        let db = Database::new(path_str(&db_path)).expect("Failed to create test db");

        let manifest_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO manifests (id, source_filename, total_retail_value, total_cost, items_count)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![manifest_id, "manifest.csv", 1000.0_f64, 140.0_f64, 1_i64],
            )
            .expect("Failed to insert manifest");

        let auction_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO auctions (id, name, status, total_lots)
                 VALUES (?1, ?2, 'Active', 1)",
                params![auction_id, "Sugarland 1005"],
            )
            .expect("Failed to insert auction");

        let item_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO inventory_items (
                    id, manifest_id, lot_number, quantity, raw_title, vendor_code, source,
                    retail_price, cost_price, min_price, current_status, auction_id
                 ) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, ?8, ?9, 'Listed', ?10)",
                params![
                    item_id,
                    manifest_id,
                    "1",
                    "Matched Item",
                    "BB-001",
                    "Best Buy",
                    1000.0_f64,
                    140.0_f64,
                    100.0_f64,
                    auction_id
                ],
            )
            .expect("Failed to insert inventory item");

        let csv_path = base_dir.join("results_unmatched.csv");
        fs::write(
            &csv_path,
            "Lot,Title,Winning Bidder,Name,High Bid,Max Bid,Email,Phone\n1,Matched Item,1001,Matched Buyer,30500,,,\n2,Unmatched Item,1002,Extra Buyer,12500,13000,,,\n",
        )
        .expect("Failed to write csv");

        let app_data_dir = base_dir.join("app_data");
        fs::create_dir_all(&app_data_dir).expect("Failed to create app data dir");

        let reports = AuctionManager::finish_auction(
            &db,
            &auction_id,
            path_str(&csv_path),
            path_str(&app_data_dir),
        )
        .expect("finish_auction failed");

        #[cfg(target_os = "windows")]
        {
            let reports_dir = app_data_dir.join("reports").join(&auction_id);
            let detail_path = reports_dir.join(&reports.detail_report);
            let detail_xml =
                extract_sheet_xml(&base_dir, "detail_unmatched", path_str(&detail_path));

            assert!(
                detail_xml.contains("dimension ref=\"A1:Z3\""),
                "Detail report should include unmatched HiBid row"
            );
        }
    }

    #[test]
    fn finish_auction_replaces_previous_reports() {
        let base_dir: PathBuf = std::env::temp_dir().join(format!(
            "sugarland_finish_auction_replace_reports_{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&base_dir).expect("Failed to create temp dir");

        let db_path = base_dir.join("replace_reports.db");
        let db = Database::new(path_str(&db_path)).expect("Failed to create test db");

        let manifest_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO manifests (id, source_filename, total_retail_value, total_cost, items_count)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![manifest_id, "manifest.csv", 1000.0_f64, 140.0_f64, 1_i64],
            )
            .expect("Failed to insert manifest");

        let auction_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO auctions (id, name, status, total_lots)
                 VALUES (?1, ?2, 'Active', 1)",
                params![auction_id, "Sugarland 1006"],
            )
            .expect("Failed to insert auction");

        let item_id = Uuid::new_v4().to_string();
        db.conn
            .execute(
                "INSERT INTO inventory_items (
                    id, manifest_id, lot_number, quantity, raw_title, vendor_code, source,
                    retail_price, cost_price, min_price, current_status, auction_id
                 ) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, ?8, ?9, 'Listed', ?10)",
                params![
                    item_id,
                    manifest_id,
                    "1",
                    "Replace Reports Item",
                    "BB-001",
                    "Best Buy",
                    1000.0_f64,
                    140.0_f64,
                    100.0_f64,
                    auction_id
                ],
            )
            .expect("Failed to insert inventory item");

        let csv_path = base_dir.join("results_replace_reports.csv");
        fs::write(
            &csv_path,
            "Lot,Title,Winning Bidder,Name,High Bid,Max Bid,Email,Phone\n1,Replace Reports Item,1001,Buyer One,30500,,,\n",
        )
        .expect("Failed to write csv");

        let app_data_dir = base_dir.join("app_data");
        fs::create_dir_all(&app_data_dir).expect("Failed to create app data dir");

        AuctionManager::finish_auction(
            &db,
            &auction_id,
            path_str(&csv_path),
            path_str(&app_data_dir),
        )
        .expect("first finish_auction failed");

        let reports_dir = app_data_dir.join("reports").join(&auction_id);
        fs::create_dir_all(&reports_dir).expect("Failed to create reports dir");
        let stale_file = reports_dir.join("stale_report.xlsx");
        fs::write(&stale_file, "stale").expect("Failed to create stale file");
        db.conn
            .execute(
                "INSERT INTO auction_reports (id, auction_id, report_type, file_name, file_path)
                 VALUES (?1, ?2, 'stale', ?3, ?4)",
                params![
                    Uuid::new_v4().to_string(),
                    auction_id,
                    "stale_report.xlsx",
                    stale_file.to_string_lossy().to_string()
                ],
            )
            .expect("Failed to insert stale report row");

        AuctionManager::finish_auction(
            &db,
            &auction_id,
            path_str(&csv_path),
            path_str(&app_data_dir),
        )
        .expect("second finish_auction failed");

        assert!(
            !stale_file.exists(),
            "Old stale report file should be removed"
        );

        let report_rows: Vec<(String, String)> = db
            .conn
            .prepare(
                "SELECT report_type, file_name
                 FROM auction_reports
                 WHERE auction_id = ?1
                 ORDER BY report_type",
            )
            .expect("Failed to prepare report query")
            .query_map(params![auction_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("Failed to query reports")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("Failed to collect reports");

        assert_eq!(report_rows.len(), 2, "Only latest 2 reports should remain");
        assert_eq!(report_rows[0].0, "detail");
        assert_eq!(report_rows[1].0, "summary");
    }
}
